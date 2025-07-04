use std::{
    collections::HashMap,
    sync::mpsc::{self, Receiver, Sender},
};

use ratatui::{
    crossterm::{
        event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
        terminal::{disable_raw_mode, enable_raw_mode},
    },
    init,
    layout::{Alignment, Constraint, Layout},
    restore,
    style::{Color, Style},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, ListState, Paragraph},
    DefaultTerminal, Frame,
};
use sysinfo::Signal;

use crate::{
    components::{
        network::draw_network_info, process::draw_process_info,
        theme::get_and_return_app_color_info,
    },
    cpu::draw_cpu_info,
    disk::draw_disk_info,
    get_sys_info::{spawn_process_info_collector, spawn_system_info_collector},
    memory::draw_memory_info,
    types::{
        AppColorInfo, AppPopUpType, AppState, CProcessesInfo, CSysInfo,
        CurrentProcessSignalStateData, MemoryData, ProcessData, ProcessSortType, ProcessesInfo,
        SelectedContainer, SysInfo,
    },
    utils::{
        get_signal_from_int, process_processes_info, process_sys_info, render_pop_up_menu,
        send_signal,
    },
};

// this need to be the same as MAXIMUM_DATA_COLLECTION in types.rs
const MAX_GRAPH_SHOWN_RANGE: usize = 500;

struct App {
    is_quit: bool,                          // to indicate is user wanted to quit the app
    tick: u32, // refresh rate for the metrics ( default is 1000ms, customizable by user )
    tx: Sender<CSysInfo>, // this will be pass to another thread that will be spawn for collecting metrics to send the data collected back
    rx: Receiver<CSysInfo>, // this will be in the main app to receive the data info send back
    process_tx: Sender<CProcessesInfo>, // this will be pass to another thread that will be spawn for collecting process metrics to send the data collected back
    process_rx: Receiver<CProcessesInfo>, // this will be in the main app to receive the process data info send back
    tick_tx: Sender<u32>, // this will be for sending the updated tick to the thread spawn to update the frequency of collecting data
    process_tick_tx: Sender<u32>, // this will be for sending the updated tick to the thread spawn to update the frequency of collecting process data
    sys_info: SysInfo,            // the system info collected
    process_info: ProcessesInfo,  // the system process info collected
    selected_container: SelectedContainer, // current selected container in the UI
    state: AppState,              // current state of the app
    pop_up_type: AppPopUpType,    // current pop up type
    cpu_graph_shown_range: usize, // range of graph shown for CPU
    memory_graph_shown_range: usize, // range of graph shown for MEMORY
    disk_graph_shown_range: usize, // range of graph shown for DISK
    network_graph_shown_range: usize, // range of graph shown for NETWORK
    process_graph_shown_range: usize, // range of graph shown for PROCESS [ this will the the graph shown in the process detail layout ]
    cpu_selected_state: ListState,    // current selected individual cpu
    disk_selected_entry: usize,       // current selected individual disk
    network_selected_entry: usize,    // current selected individual network
    process_current_list: Vec<ProcessData>, // current process list after filtering/sorting
    process_selectable_entries: usize, // current selectable entries in the process list
    process_selected_state: ListState, // current selected individual process
    process_sort_selected_state: u8,  // current selected sorting
    process_sort_type: ProcessSortType, // current sorting type
    process_sort_is_reversed: bool, // by default the sorting will be in descending order (true), by setting this to false, the sort will be in ascending order
    process_filter: String,         // current user input for filtering
    process_show_details: bool,     // indicate if user wanted to show process details
    current_showing_process_detail: Option<HashMap<String, ProcessData>>, // the current showing process detail
    is_renderable: bool,         // to indicate if this app UI is renderable
    is_init: bool,               // to indicate is this app has done initialization
    container_full_screen: bool, // to indicate is user choose to full screen the current selected container
    current_process_signal_state_data: Option<CurrentProcessSignalStateData>, // this was used to temporary save the data when user trigger the process signal related pop-up
}

const MIN_HEIGHT: u16 = 25;
const MIN_WIDTH: u16 = 90;

pub fn app() {
    enable_raw_mode().unwrap();
    let mut terminal = init();
    let (tx, rx) = mpsc::channel();
    let (process_tx, process_rx) = mpsc::channel();
    let (tick_tx, tick_rx) = mpsc::channel();
    let (process_tick_tx, process_tick_rx) = mpsc::channel();

    let mut app = App {
        is_quit: false,
        tick: 1000,
        tx,
        rx,
        process_tx,
        process_rx,
        tick_tx,
        process_tick_tx,
        sys_info: SysInfo {
            cpus: vec![],
            memory: MemoryData::default(),
            disks: HashMap::new(),
            networks: HashMap::new(),
        },
        process_info: ProcessesInfo {
            processes: HashMap::new(),
        },
        selected_container: SelectedContainer::None,
        state: AppState::View,
        pop_up_type: AppPopUpType::None,
        cpu_graph_shown_range: 100,
        memory_graph_shown_range: 100,
        disk_graph_shown_range: 100,
        network_graph_shown_range: 100,
        process_graph_shown_range: 100,
        cpu_selected_state: ListState::default(),
        disk_selected_entry: 0,
        network_selected_entry: 0,
        process_current_list: vec![],
        process_selectable_entries: 0,
        process_selected_state: ListState::default(),
        process_sort_selected_state: 0,
        process_sort_type: ProcessSortType::Thread,
        process_sort_is_reversed: true,
        process_filter: String::new(),
        process_show_details: false,
        current_showing_process_detail: None,
        is_renderable: true,
        is_init: false,
        container_full_screen: false,
        current_process_signal_state_data: None,
    };

    let app_color_info = get_and_return_app_color_info();
    app.run(&mut terminal, tick_rx, process_tick_rx, app_color_info);
    disable_raw_mode().unwrap();
    restore();
}

impl App {
    // runs the application's main loop until the user quits
    pub fn run(
        &mut self,
        terminal: &mut DefaultTerminal,
        tick_rx: Receiver<u32>,
        process_tick_rx: Receiver<u32>,
        app_color_info: AppColorInfo,
    ) {
        // when the program start, we let the info collector to collect at 100ms
        // only after the initial collection, we reset to the user selected tick ( this will be able to be configure at a later stage )
        spawn_system_info_collector(tick_rx, self.tx.clone(), 100);
        spawn_process_info_collector(process_tick_rx, self.process_tx.clone(), 100);

        while !self.is_init {
            match self.rx.try_recv() {
                Ok(c_sys_info) => {
                    process_sys_info(&mut self.sys_info, c_sys_info);
                    match self.process_rx.try_recv() {
                        Ok(c_processes_info) => {
                            process_processes_info(
                                &mut self.process_info,
                                c_processes_info,
                                &mut self.current_showing_process_detail,
                            );
                            self.is_init = true;
                        }
                        Err(_) => {
                            self.is_init = false;
                        }
                    }
                }
                Err(_) => {
                    self.is_init = false;
                }
            }
        }
        self.cpu_selected_state.select(Some(0));

        self.process_selectable_entries = self.process_info.processes.len();
        self.process_selected_state.select(None);
        let _ = self.tick_tx.send(self.tick);
        let _ = self.process_tick_tx.send(self.tick);

        while !self.is_quit {
            let c_sys_info = self.rx.try_recv();
            if c_sys_info.is_ok() {
                process_sys_info(&mut self.sys_info, c_sys_info.unwrap());
            }

            let c_process_info = self.process_rx.try_recv();
            if c_process_info.is_ok() {
                process_processes_info(
                    &mut self.process_info,
                    c_process_info.unwrap(),
                    &mut self.current_showing_process_detail,
                );
            }
            let _ = terminal.draw(|frame| self.draw(frame, &app_color_info));

            // we only handle event if the tui is renderable
            if self.is_renderable {
                self.handle_events();
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame, app_color_info: &AppColorInfo) {
        //
        //                       The TUI Layout
        //
        //   ------------------------------------------------------------
        //   |                                                          |
        //   |                  CPU INFO (top 30.0%)                    |
        //   |                                                          |
        //   ------------------------------------------------------------
        //   |   (MEMORY AND DIKS)     |                                |
        //   |    Bottom left (45%)    |   (PROCESS bottom right 55%)   |
        //   |      & top (65%)        |                                |
        //   |--------------------(BOTTOM 70%)                          |
        //   |      (NETWORK)          |                                |
        //   |    Bottom left (45%)    |                                |
        //   |     & bottom (35%)      |                                |
        //   ------------------------------------------------------------

        // split and init the layout space for each container
        let top_and_bottom = Layout::vertical([Constraint::Fill(30), Constraint::Fill(70)]);
        let [cpu_area, bottom] = top_and_bottom.areas(frame.area());
        let [bottom_left, process_area] =
            Layout::horizontal([Constraint::Fill(45), Constraint::Fill(55)]).areas(bottom);
        let [memory_disk_area, network_area] =
            Layout::vertical([Constraint::Fill(65), Constraint::Fill(35)]).areas(bottom_left);
        let [memory_area, disk_area] =
            Layout::horizontal([Constraint::Fill(50), Constraint::Fill(50)])
                .areas(memory_disk_area);

        // set the bg
        let background =
            Block::default().style(Style::default().bg(app_color_info.background_color)); // Set your desired background color
        frame.render_widget(background, frame.area());

        // check if the terminal size is valid
        let full_frame_view_rect = frame.area();
        if full_frame_view_rect.width < MIN_WIDTH || full_frame_view_rect.height < MIN_HEIGHT {
            self.is_renderable = false;
            draw_not_renderable_message(frame, app_color_info);
            return;
        } else {
            self.is_renderable = true;
        }

        if self.is_renderable {
            // we check the selcted disk entry to prevent selecting a disk that got removed
            //
            // default to the first disk entry
            let mut selected_disk = self.sys_info.disks.iter().nth(0).unwrap().1;
            // if the selected disk is valid, override the selected default disk
            if let Some((_, value)) = self.sys_info.disks.iter().nth(self.disk_selected_entry) {
                selected_disk = value;
            } else {
                self.disk_selected_entry = 0;
            }

            // default to the first network entry
            let mut selected_network = self.sys_info.networks.iter().nth(0).unwrap().1;
            // if the selected network is valid, override the selected default network
            if let Some((_, value)) = self
                .sys_info
                .networks
                .iter()
                .nth(self.network_selected_entry)
            {
                selected_network = value;
            } else {
                self.network_selected_entry = 0;
            }

            // handling for full screen mode
            if self.container_full_screen {
                if self.selected_container == SelectedContainer::Cpu {
                    draw_cpu_info(
                        self.tick as u64,
                        &self.sys_info.cpus,
                        full_frame_view_rect,
                        frame,
                        &mut self.cpu_selected_state,
                        self.cpu_graph_shown_range,
                        if self.selected_container == SelectedContainer::Cpu {
                            true
                        } else {
                            false
                        },
                        app_color_info,
                    );
                } else if self.selected_container == SelectedContainer::Memory {
                    draw_memory_info(
                        self.tick as u64,
                        &self.sys_info.memory,
                        full_frame_view_rect,
                        frame,
                        self.memory_graph_shown_range,
                        if self.selected_container == SelectedContainer::Memory {
                            true
                        } else {
                            false
                        },
                        app_color_info,
                        true,
                    )
                } else if self.selected_container == SelectedContainer::Disk {
                    draw_disk_info(
                        self.tick as u64,
                        &selected_disk,
                        full_frame_view_rect,
                        frame,
                        self.disk_graph_shown_range,
                        if self.selected_container == SelectedContainer::Disk {
                            true
                        } else {
                            false
                        },
                        app_color_info,
                        true,
                    )
                } else if self.selected_container == SelectedContainer::Network {
                    draw_network_info(
                        self.tick as u64,
                        &selected_network,
                        full_frame_view_rect,
                        frame,
                        self.network_graph_shown_range,
                        if self.selected_container == SelectedContainer::Network {
                            true
                        } else {
                            false
                        },
                        app_color_info,
                        true,
                    )
                } else if self.selected_container == SelectedContainer::Process {
                    draw_process_info(
                        self.tick as u64,
                        &self.process_info.processes,
                        &mut self.process_current_list,
                        &mut self.process_selectable_entries,
                        &mut self.process_selected_state,
                        &self.process_sort_type,
                        self.process_sort_is_reversed,
                        self.process_filter.clone(),
                        self.process_show_details,
                        &self.current_showing_process_detail,
                        self.sys_info.memory.total_memory,
                        self.state == AppState::Typing,
                        full_frame_view_rect,
                        frame,
                        self.process_graph_shown_range,
                        if self.selected_container == SelectedContainer::Process {
                            true
                        } else {
                            false
                        },
                        app_color_info,
                        true,
                    )
                }
            } else {
                draw_cpu_info(
                    self.tick as u64,
                    &self.sys_info.cpus,
                    cpu_area,
                    frame,
                    &mut self.cpu_selected_state,
                    self.cpu_graph_shown_range,
                    if self.selected_container == SelectedContainer::Cpu {
                        true
                    } else {
                        false
                    },
                    app_color_info,
                );

                draw_memory_info(
                    self.tick as u64,
                    &self.sys_info.memory,
                    memory_area,
                    frame,
                    self.memory_graph_shown_range,
                    if self.selected_container == SelectedContainer::Memory {
                        true
                    } else {
                        false
                    },
                    app_color_info,
                    false,
                );

                draw_disk_info(
                    self.tick as u64,
                    &selected_disk,
                    disk_area,
                    frame,
                    self.disk_graph_shown_range,
                    if self.selected_container == SelectedContainer::Disk {
                        true
                    } else {
                        false
                    },
                    app_color_info,
                    false,
                );

                draw_network_info(
                    self.tick as u64,
                    &selected_network,
                    network_area,
                    frame,
                    self.network_graph_shown_range,
                    if self.selected_container == SelectedContainer::Network {
                        true
                    } else {
                        false
                    },
                    app_color_info,
                    false,
                );

                draw_process_info(
                    self.tick as u64,
                    &self.process_info.processes,
                    &mut self.process_current_list,
                    &mut self.process_selectable_entries,
                    &mut self.process_selected_state,
                    &self.process_sort_type,
                    self.process_sort_is_reversed,
                    self.process_filter.clone(),
                    self.process_show_details,
                    &self.current_showing_process_detail,
                    self.sys_info.memory.total_memory,
                    self.state == AppState::Typing,
                    process_area,
                    frame,
                    self.process_graph_shown_range,
                    if self.selected_container == SelectedContainer::Process {
                        true
                    } else {
                        false
                    },
                    app_color_info,
                    false,
                )
            }

            // render pop up after all the main components are rendered
            // for the pop up size, it will be decide at the function according to the pop up type
            if self.state == AppState::Popup && self.pop_up_type != AppPopUpType::None {
                render_pop_up_menu(
                    full_frame_view_rect,
                    frame,
                    &mut self.pop_up_type,
                    self.current_process_signal_state_data.as_ref().unwrap(),
                    app_color_info,
                );
            }
        }
    }

    fn handle_events(&mut self) {
        if event::poll(std::time::Duration::from_millis(100)).unwrap() {
            match event::read().unwrap() {
                // it's important to check that the event is a key press event as
                // crossterm also emits key release and repeat events on Windows.
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    if self.state == AppState::View {
                        self.handle_key_event(key_event);
                    } else if self.state == AppState::Typing {
                        self.handle_typing_key_event(key_event);
                    } else if self.state == AppState::Popup {
                        self.handle_pop_up_event(key_event);
                    }
                }
                _ => {}
            };
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                if self.state == AppState::View {
                    // quit the ratatui terminal user interface
                    if self.selected_container == SelectedContainer::None {
                        self.is_quit = true;
                    } else {
                        if self.container_full_screen {
                            self.container_full_screen = false;
                        } else {
                            self.selected_container = SelectedContainer::None;
                        }
                    }
                }
            }

            KeyCode::Char('-') => {
                if self.state == AppState::View {
                    if self.tick > 100 {
                        self.tick -= 100;
                        self.tick_tx.send(self.tick).unwrap();
                        self.process_tick_tx.send(self.tick).unwrap();
                    }
                }
            }
            KeyCode::Char('+') => {
                if self.state == AppState::View {
                    if self.tick < 10000 {
                        self.tick += 100;
                        self.tick_tx.send(self.tick).unwrap();
                        self.process_tick_tx.send(self.tick).unwrap();
                    }
                }
            }

            KeyCode::Up => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Cpu {
                        if let Some(selected) = self.cpu_selected_state.selected() {
                            if selected > 0 {
                                self.cpu_selected_state.select(Some(selected - 1));
                            } else {
                                self.cpu_selected_state
                                    .select(Some(self.sys_info.cpus.len() - 1))
                            }
                        }
                    } else if self.selected_container == SelectedContainer::Process {
                        if let Some(selected) = self.process_selected_state.selected() {
                            if selected > 0 {
                                self.process_selected_state.select(Some(selected - 1));
                            } else {
                                self.process_selected_state.select(None);
                            }
                        }
                    }
                }
            }
            KeyCode::Down => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Cpu {
                        if let Some(selected) = self.cpu_selected_state.selected() {
                            if selected < self.sys_info.cpus.len().saturating_sub(1) {
                                self.cpu_selected_state.select(Some(selected + 1));
                            } else {
                                self.cpu_selected_state.select(Some(0))
                            }
                        }
                    } else if self.selected_container == SelectedContainer::Process {
                        if let Some(selected) = self.process_selected_state.selected() {
                            if selected < self.process_selectable_entries.saturating_sub(1) {
                                self.process_selected_state.select(Some(selected + 1));
                            }
                        } else {
                            self.process_selected_state.select(Some(0))
                        }
                    }
                }
            }
            KeyCode::Char('[') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Cpu {
                        if self.cpu_graph_shown_range > 100 {
                            self.cpu_graph_shown_range -= 10;
                        }
                    } else if self.selected_container == SelectedContainer::Memory {
                        if self.memory_graph_shown_range > 100 {
                            self.memory_graph_shown_range -= 10;
                        }
                    } else if self.selected_container == SelectedContainer::Disk {
                        if self.disk_graph_shown_range > 100 {
                            self.disk_graph_shown_range -= 10;
                        }
                    } else if self.selected_container == SelectedContainer::Network {
                        if self.network_graph_shown_range > 100 {
                            self.network_graph_shown_range -= 10;
                        }
                    } else if self.selected_container == SelectedContainer::Process {
                        if self.process_graph_shown_range > 100 {
                            self.process_graph_shown_range -= 10;
                        }
                    } else if self.selected_container == SelectedContainer::None {
                        if self.cpu_graph_shown_range > 100 {
                            self.cpu_graph_shown_range -= 10;
                        }
                        if self.memory_graph_shown_range > 100 {
                            self.memory_graph_shown_range -= 10;
                        }
                        if self.disk_graph_shown_range > 100 {
                            self.disk_graph_shown_range -= 10;
                        }
                        if self.network_graph_shown_range > 100 {
                            self.network_graph_shown_range -= 10;
                        }
                        if self.process_graph_shown_range > 100 {
                            self.process_graph_shown_range -= 10;
                        }
                    }
                }
            }

            KeyCode::Char(']') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Cpu {
                        if self.cpu_graph_shown_range < MAX_GRAPH_SHOWN_RANGE {
                            self.cpu_graph_shown_range += 10;
                        }
                    } else if self.selected_container == SelectedContainer::Memory {
                        if self.memory_graph_shown_range < MAX_GRAPH_SHOWN_RANGE {
                            self.memory_graph_shown_range += 10;
                        }
                    } else if self.selected_container == SelectedContainer::Disk {
                        if self.disk_graph_shown_range < MAX_GRAPH_SHOWN_RANGE {
                            self.disk_graph_shown_range += 10;
                        }
                    } else if self.selected_container == SelectedContainer::Network {
                        if self.network_graph_shown_range < MAX_GRAPH_SHOWN_RANGE {
                            self.network_graph_shown_range += 10;
                        }
                    } else if self.selected_container == SelectedContainer::Process {
                        if self.process_graph_shown_range < MAX_GRAPH_SHOWN_RANGE {
                            self.process_graph_shown_range += 10;
                        }
                    } else if self.selected_container == SelectedContainer::None {
                        if self.cpu_graph_shown_range < MAX_GRAPH_SHOWN_RANGE {
                            self.cpu_graph_shown_range += 10;
                        }
                        if self.memory_graph_shown_range < MAX_GRAPH_SHOWN_RANGE {
                            self.memory_graph_shown_range += 10;
                        }
                        if self.disk_graph_shown_range < MAX_GRAPH_SHOWN_RANGE {
                            self.disk_graph_shown_range += 10;
                        }
                        if self.network_graph_shown_range < MAX_GRAPH_SHOWN_RANGE {
                            self.network_graph_shown_range += 10;
                        }
                        if self.process_graph_shown_range < MAX_GRAPH_SHOWN_RANGE {
                            self.process_graph_shown_range += 10;
                        }
                    }
                }
            }

            // c and C for selecting the Cpu Block
            KeyCode::Char('c') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::None
                        || self.selected_container != SelectedContainer::Cpu
                    {
                        self.selected_container = SelectedContainer::Cpu;
                    } else {
                        self.container_full_screen = false;
                        self.selected_container = SelectedContainer::None;
                    }
                }
            }
            KeyCode::Char('C') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::None
                        || self.selected_container != SelectedContainer::Cpu
                    {
                        self.selected_container = SelectedContainer::Cpu;
                    } else {
                        self.container_full_screen = false;
                        self.selected_container = SelectedContainer::None;
                    }
                }
            }

            // m and M for selecting the Memory Block
            KeyCode::Char('m') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::None
                        || self.selected_container != SelectedContainer::Memory
                    {
                        self.selected_container = SelectedContainer::Memory;
                    } else {
                        self.container_full_screen = false;
                        self.selected_container = SelectedContainer::None;
                    }
                }
            }
            KeyCode::Char('M') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::None
                        || self.selected_container != SelectedContainer::Memory
                    {
                        self.selected_container = SelectedContainer::Memory;
                    } else {
                        self.container_full_screen = false;
                        self.selected_container = SelectedContainer::None;
                    }
                }
            }

            // d and D for selecting the Disk Block
            KeyCode::Char('d') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::None
                        || self.selected_container != SelectedContainer::Disk
                    {
                        self.selected_container = SelectedContainer::Disk;
                    } else {
                        self.container_full_screen = false;
                        self.selected_container = SelectedContainer::None;
                    }
                }
            }
            KeyCode::Char('D') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::None
                        || self.selected_container != SelectedContainer::Disk
                    {
                        self.selected_container = SelectedContainer::Disk;
                    } else {
                        self.container_full_screen = false;
                        self.selected_container = SelectedContainer::None;
                    }
                }
            }

            // n and N for selecting the Disk Block
            KeyCode::Char('n') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::None
                        || self.selected_container != SelectedContainer::Network
                    {
                        self.selected_container = SelectedContainer::Network;
                    } else {
                        self.container_full_screen = false;
                        self.selected_container = SelectedContainer::None;
                    }
                }
            }
            KeyCode::Char('N') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::None
                        || self.selected_container != SelectedContainer::Network
                    {
                        self.selected_container = SelectedContainer::Network;
                    } else {
                        self.container_full_screen = false;
                        self.selected_container = SelectedContainer::None;
                    }
                }
            }

            // p and P for selecting the Process Block
            KeyCode::Char('p') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::None
                        || self.selected_container != SelectedContainer::Process
                    {
                        self.selected_container = SelectedContainer::Process;
                    } else {
                        self.container_full_screen = false;
                        self.selected_container = SelectedContainer::None;
                    }
                }
            }
            KeyCode::Char('P') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::None
                        || self.selected_container != SelectedContainer::Process
                    {
                        self.selected_container = SelectedContainer::Process;
                    } else {
                        self.container_full_screen = false;
                        self.selected_container = SelectedContainer::None;
                    }
                }
            }

            KeyCode::Char('R') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Process {
                        if self.process_sort_is_reversed {
                            self.process_sort_is_reversed = false;
                        } else {
                            self.process_sort_is_reversed = true;
                        }
                    }
                }
            }

            KeyCode::Char('r') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Process {
                        if self.process_sort_is_reversed {
                            self.process_sort_is_reversed = false;
                        } else {
                            self.process_sort_is_reversed = true;
                        }
                    }
                }
            }

            KeyCode::Char('f') => {
                if self.state == AppState::View {
                    self.state = AppState::Typing;
                    if self.process_filter.is_empty() || self.process_filter == "_".to_string() {
                        self.process_filter = "_".to_string();
                    }
                }
            }

            KeyCode::Char('F') => {
                if self.state == AppState::View {
                    self.state = AppState::Typing;
                    if self.process_filter.is_empty() || self.process_filter == "_".to_string() {
                        self.process_filter = "_".to_string();
                    }
                }
            }

            KeyCode::Char('K') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Process
                        && self.process_show_details
                        && self.current_showing_process_detail.is_some()
                        && self.process_selected_state.selected().is_none()
                    {
                        let (key, value) = self
                            .current_showing_process_detail
                            .as_ref()
                            .unwrap()
                            .iter()
                            .next()
                            .unwrap();
                        // do nothing if the status is killed
                        if value.status == "killed" {
                            return;
                        }
                        let program_pib = key.clone();
                        let program_name = value.name.clone();
                        self.current_process_signal_state_data =
                            Some(CurrentProcessSignalStateData {
                                pid: program_pib,
                                name: program_name,
                                signal: Some(Signal::Kill),
                                signal_id: Some(9),
                                yes_confirmation: true,
                                no_confirmation: false,
                            });
                        self.state = AppState::Popup;
                        self.pop_up_type = AppPopUpType::KillConfirmation;
                    }
                }
            }

            KeyCode::Char('k') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Process
                        && self.process_show_details
                        && self.current_showing_process_detail.is_some()
                        && self.process_selected_state.selected().is_none()
                    {
                        let (key, value) = self
                            .current_showing_process_detail
                            .as_ref()
                            .unwrap()
                            .iter()
                            .next()
                            .unwrap();
                        // do nothing if the status is killed
                        if value.status == "killed" {
                            return;
                        }

                        let program_pib = key.clone();
                        let program_name = value.name.clone();
                        self.current_process_signal_state_data =
                            Some(CurrentProcessSignalStateData {
                                pid: program_pib,
                                name: program_name,
                                signal: Some(Signal::Kill),
                                signal_id: Some(9),
                                yes_confirmation: true,
                                no_confirmation: false,
                            });
                        self.state = AppState::Popup;
                        self.pop_up_type = AppPopUpType::KillConfirmation;
                    }
                }
            }

            KeyCode::Char('T') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Process
                        && self.process_show_details
                        && self.current_showing_process_detail.is_some()
                        && self.process_selected_state.selected().is_none()
                    {
                        let (key, value) = self
                            .current_showing_process_detail
                            .as_ref()
                            .unwrap()
                            .iter()
                            .next()
                            .unwrap();
                        // do nothing if the status is killed
                        if value.status == "killed" {
                            return;
                        }

                        let program_pib = key.clone();
                        let program_name = value.name.clone();
                        self.current_process_signal_state_data =
                            Some(CurrentProcessSignalStateData {
                                pid: program_pib,
                                name: program_name,
                                signal: Some(Signal::Term),
                                signal_id: Some(15),
                                yes_confirmation: true,
                                no_confirmation: false,
                            });
                        self.state = AppState::Popup;
                        self.pop_up_type = AppPopUpType::TerminateConfirmation;
                    }
                }
            }

            KeyCode::Char('t') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Process
                        && self.process_show_details
                        && self.current_showing_process_detail.is_some()
                        && self.process_selected_state.selected().is_none()
                    {
                        let (key, value) = self
                            .current_showing_process_detail
                            .as_ref()
                            .unwrap()
                            .iter()
                            .next()
                            .unwrap();
                        // do nothing if the status is killed
                        if value.status == "killed" {
                            return;
                        }

                        let program_pib = key.clone();
                        let program_name = value.name.clone();
                        self.current_process_signal_state_data =
                            Some(CurrentProcessSignalStateData {
                                pid: program_pib,
                                name: program_name,
                                signal: Some(Signal::Term),
                                signal_id: Some(15),
                                yes_confirmation: true,
                                no_confirmation: false,
                            });
                        self.state = AppState::Popup;
                        self.pop_up_type = AppPopUpType::TerminateConfirmation;
                    }
                }
            }

            KeyCode::Char('S') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Process
                        && self.process_show_details
                        && self.current_showing_process_detail.is_some()
                        && self.process_selected_state.selected().is_none()
                    {
                        let (key, value) = self
                            .current_showing_process_detail
                            .as_ref()
                            .unwrap()
                            .iter()
                            .next()
                            .unwrap();
                        // do nothing if the status is killed
                        if value.status == "killed" {
                            return;
                        }

                        let program_pib = key.clone();
                        let program_name = value.name.clone();

                        self.current_process_signal_state_data =
                            Some(CurrentProcessSignalStateData {
                                pid: program_pib,
                                signal: None,
                                signal_id: None,
                                name: program_name,
                                yes_confirmation: true,
                                no_confirmation: false,
                            });
                        self.state = AppState::Popup;
                        self.pop_up_type = AppPopUpType::SignalMenu;
                    }
                }
            }

            KeyCode::Char('s') => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Process
                        && self.process_show_details
                        && self.current_showing_process_detail.is_some()
                        && self.process_selected_state.selected().is_none()
                    {
                        let (key, value) = self
                            .current_showing_process_detail
                            .as_ref()
                            .unwrap()
                            .iter()
                            .next()
                            .unwrap();
                        // do nothing if the status is killed
                        if value.status == "killed" {
                            return;
                        }

                        let program_pib = key.clone();
                        let program_name = value.name.clone();

                        self.current_process_signal_state_data =
                            Some(CurrentProcessSignalStateData {
                                pid: program_pib,
                                signal: None,
                                signal_id: None,
                                name: program_name,
                                yes_confirmation: true,
                                no_confirmation: false,
                            });
                        self.state = AppState::Popup;
                        self.pop_up_type = AppPopUpType::SignalMenu;
                    }
                }
            }

            KeyCode::Left => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Disk {
                        if self.disk_selected_entry == 0 {
                            self.disk_selected_entry = self.sys_info.disks.len() - 1;
                        } else {
                            self.disk_selected_entry -= 1;
                        }
                    } else if self.selected_container == SelectedContainer::Network {
                        if self.network_selected_entry == 0 {
                            self.network_selected_entry = self.sys_info.networks.len() - 1;
                        } else {
                            self.network_selected_entry -= 1;
                        }
                    } else if self.selected_container == SelectedContainer::Process {
                        if self.process_sort_selected_state == 0 {
                            self.process_sort_selected_state =
                                ProcessSortType::total_selection_count() - 1;
                        } else {
                            self.process_sort_selected_state -= 1;
                        }
                        self.process_sort_type = ProcessSortType::get_process_sort_type_from_int(
                            self.process_sort_selected_state,
                        )
                    }
                }
            }
            KeyCode::Right => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Disk {
                        if self.disk_selected_entry == self.sys_info.disks.len() - 1 {
                            self.disk_selected_entry = 0
                        } else {
                            self.disk_selected_entry += 1;
                        }
                    } else if self.selected_container == SelectedContainer::Network {
                        if self.network_selected_entry == self.sys_info.networks.len() - 1 {
                            self.network_selected_entry = 0;
                        } else {
                            self.network_selected_entry += 1;
                        }
                    } else if self.selected_container == SelectedContainer::Process {
                        if self.process_sort_selected_state
                            == ProcessSortType::total_selection_count() - 1
                        {
                            self.process_sort_selected_state = 0;
                        } else {
                            self.process_sort_selected_state += 1;
                        }
                        self.process_sort_type = ProcessSortType::get_process_sort_type_from_int(
                            self.process_sort_selected_state,
                        )
                    }
                }
            }

            KeyCode::Backspace => {
                if self.state == AppState::View {
                    self.process_filter = "".to_string();
                    self.process_selected_state.select(None);
                }
            }

            KeyCode::Tab => {
                if self.state == AppState::View {
                    // for a container to be full screen, it need to be selected first
                    if self.container_full_screen
                        && self.selected_container != SelectedContainer::None
                    {
                        self.container_full_screen = false;
                    } else if !self.container_full_screen
                        && self.selected_container != SelectedContainer::None
                    {
                        self.container_full_screen = true;
                    }
                }
            }

            KeyCode::Enter => {
                if self.state == AppState::View {
                    if self.selected_container == SelectedContainer::Process {
                        if let Some(selected) = self.process_selected_state.selected() {
                            self.process_show_details = true;
                            let mut selected_process = HashMap::new();
                            selected_process.insert(
                                self.process_current_list[selected].pid.to_string(),
                                self.process_current_list[selected].clone(),
                            );
                            self.current_showing_process_detail = Some(selected_process);

                            // unselect current selected process item list to enter the process detail container
                            self.process_selected_state.select(None);
                        } else {
                            self.process_show_details = false;
                            self.current_showing_process_detail = None;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_typing_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Backspace => {
                if !self.process_filter.is_empty() && self.process_filter != "_".to_string() {
                    self.process_filter.remove(self.process_filter.len() - 2); // there will be a "_" character at the end and we don't want to remove that
                    self.process_selected_state.select(None);
                }
            }

            KeyCode::Enter => {
                self.state = AppState::View;
            }

            KeyCode::Down => {
                self.state = AppState::View;
                self.process_selected_state.select(Some(0));
            }

            KeyCode::Esc => {
                self.state = AppState::View;
            }

            KeyCode::Char(c) => {
                self.process_filter.insert(self.process_filter.len() - 1, c); // there will be a "_" character at the end and we want to insert the newly typed character before it
                self.process_selected_state.select(None);
            }

            _ => {}
        }
    }

    fn handle_pop_up_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                self.state = AppState::View;
                self.pop_up_type = AppPopUpType::None;
                self.current_process_signal_state_data = None;
            }
            KeyCode::Char('y') => {
                if self
                    .current_process_signal_state_data
                    .as_ref()
                    .unwrap()
                    .signal
                    .is_some()
                {
                    let pid = self
                        .current_process_signal_state_data
                        .as_ref()
                        .unwrap()
                        .pid
                        .parse::<usize>()
                        .unwrap();
                    let signal = self
                        .current_process_signal_state_data
                        .as_ref()
                        .unwrap()
                        .signal
                        .unwrap();
                    send_signal(pid, signal);
                }
                self.state = AppState::View;
                self.pop_up_type = AppPopUpType::None;
                self.current_process_signal_state_data = None;
            }
            KeyCode::Char('Y') => {
                if self
                    .current_process_signal_state_data
                    .as_ref()
                    .unwrap()
                    .signal
                    .is_some()
                {
                    let pid = self
                        .current_process_signal_state_data
                        .as_ref()
                        .unwrap()
                        .pid
                        .parse::<usize>()
                        .unwrap();
                    let signal = self
                        .current_process_signal_state_data
                        .as_ref()
                        .unwrap()
                        .signal
                        .unwrap();
                    send_signal(pid, signal);
                }
                self.state = AppState::View;
                self.pop_up_type = AppPopUpType::None;
                self.current_process_signal_state_data = None;
            }
            KeyCode::Char('n') => {
                self.state = AppState::View;
                self.pop_up_type = AppPopUpType::None;
                self.current_process_signal_state_data = None;
            }
            KeyCode::Char('N') => {
                self.state = AppState::View;
                self.pop_up_type = AppPopUpType::None;
                self.current_process_signal_state_data = None;
            }
            KeyCode::Left => {
                self.current_process_signal_state_data
                    .as_mut()
                    .unwrap()
                    .yes_confirmation = true;
                self.current_process_signal_state_data
                    .as_mut()
                    .unwrap()
                    .no_confirmation = false;
            }
            KeyCode::Right => {
                self.current_process_signal_state_data
                    .as_mut()
                    .unwrap()
                    .yes_confirmation = false;
                self.current_process_signal_state_data
                    .as_mut()
                    .unwrap()
                    .no_confirmation = true;
            }
            KeyCode::Enter => {
                let yes_confirmation = self
                    .current_process_signal_state_data
                    .as_ref()
                    .unwrap()
                    .yes_confirmation;
                let no_confirmation = self
                    .current_process_signal_state_data
                    .as_ref()
                    .unwrap()
                    .no_confirmation;

                if yes_confirmation
                    && !no_confirmation
                    && self
                        .current_process_signal_state_data
                        .as_ref()
                        .unwrap()
                        .signal
                        .is_some()
                {
                    let pid = self
                        .current_process_signal_state_data
                        .as_ref()
                        .unwrap()
                        .pid
                        .parse::<usize>()
                        .unwrap();
                    let signal = self
                        .current_process_signal_state_data
                        .as_ref()
                        .unwrap()
                        .signal
                        .unwrap();
                    send_signal(pid, signal);
                }
                self.state = AppState::View;
                self.pop_up_type = AppPopUpType::None;
                self.current_process_signal_state_data = None
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if self
                    .current_process_signal_state_data
                    .as_ref()
                    .unwrap()
                    .signal_id
                    .is_none()
                {
                    self.current_process_signal_state_data
                        .as_mut()
                        .unwrap()
                        .signal_id = Some(c.to_digit(10).unwrap() as u16);
                } else {
                    let mut current_signal_id_string = self
                        .current_process_signal_state_data
                        .as_ref()
                        .unwrap()
                        .signal_id
                        .unwrap()
                        .to_string();
                    current_signal_id_string.push(c);

                    let new_signal_id: u16 = current_signal_id_string.parse().unwrap();
                    if new_signal_id > 0 && new_signal_id <= 30 {
                        self.current_process_signal_state_data
                            .as_mut()
                            .unwrap()
                            .signal_id = Some(new_signal_id);
                    }
                }

                self.current_process_signal_state_data
                    .as_mut()
                    .unwrap()
                    .signal = Some(get_signal_from_int(
                    self.current_process_signal_state_data
                        .as_mut()
                        .unwrap()
                        .signal_id
                        .unwrap(),
                ))
            }
            KeyCode::Backspace => {
                if !self
                    .current_process_signal_state_data
                    .as_ref()
                    .unwrap()
                    .signal_id
                    .is_none()
                {
                    if self
                        .current_process_signal_state_data
                        .as_ref()
                        .unwrap()
                        .signal_id
                        .unwrap()
                        .to_string()
                        .len()
                        == 1
                    {
                        self.current_process_signal_state_data
                            .as_mut()
                            .unwrap()
                            .signal_id = None;
                        self.current_process_signal_state_data
                            .as_mut()
                            .unwrap()
                            .signal = None;
                    } else {
                        let mut new_signal_id_string = self
                            .current_process_signal_state_data
                            .as_ref()
                            .unwrap()
                            .signal_id
                            .unwrap()
                            .to_string();
                        new_signal_id_string.pop();

                        self.current_process_signal_state_data
                            .as_mut()
                            .unwrap()
                            .signal_id = Some(new_signal_id_string.parse::<u16>().unwrap());
                        self.current_process_signal_state_data
                            .as_mut()
                            .unwrap()
                            .signal = Some(get_signal_from_int(
                            new_signal_id_string.parse::<u16>().unwrap(),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

fn draw_not_renderable_message(frame: &mut Frame, app_color_info: &AppColorInfo) {
    let block = Block::bordered()
        .style(Color::LightYellow)
        .border_set(border::ROUNDED);

    let view_rect = frame.area();
    let height = view_rect.height;
    let width = view_rect.width;

    // Define multiple paragraphs
    let text_lines = vec![
        Line::from("UI can't be rendered, terminal size too small")
            .style(app_color_info.base_app_text_color),
        Line::from(vec![
            Span::styled(
                "Width =",
                Style::default().fg(app_color_info.base_app_text_color),
            ),
            Span::styled(
                format!(" {} ", width),
                Style::default().fg(if width >= MIN_WIDTH {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
            Span::styled(
                "Height =",
                Style::default().fg(app_color_info.base_app_text_color),
            ),
            Span::styled(
                format!(" {} ", height),
                Style::default().fg(if height >= MIN_HEIGHT {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
        ]),
        Line::from(""),
        Line::from("Need Size for current config.").style(app_color_info.base_app_text_color),
        Line::from(format!("Width = {} Height = {}  ", MIN_WIDTH, MIN_HEIGHT))
            .style(app_color_info.base_app_text_color),
    ];

    let warning_paragraph = Paragraph::new(text_lines)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(ratatui::widgets::Wrap { trim: true });

    frame.render_widget(warning_paragraph, frame.area());
}
