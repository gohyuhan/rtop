use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sysinfo::Signal;

#[derive(Serialize, Deserialize)]
pub struct ThemeConfig {
    pub theme: String,
}

// the main type structture for the application
pub struct SysInfo {
    pub cpus: Vec<CpuData>,
    pub memory: MemoryData,
    pub disks: HashMap<String, DiskData>,
    pub networks: HashMap<String, NetworkData>,
}

pub struct ProcessesInfo {
    pub processes: HashMap<String, ProcessData>, // as a hashmap to easily update existing data by retrieving it based on PID which is the key
}

pub struct AppColorInfo {
    pub background_color: Color,
    pub base_app_text_color: Color,
    // key text was the key that triggers certain functionality, like c for selecting cpu container -/+ to chnage the refresh tick
    pub key_text_color: Color,
    pub app_title_color: Color, // this will be used for those text in the title of each main block
    pub pop_up_color: Color,
    pub pop_up_selected_color_bg: Color,
    pub pop_up_blur_bg: Color,

    // for cpu
    pub cpu_container_selected_color: Color,
    pub cpu_main_block_color: Color,
    pub cpu_selected_color: Color,
    pub cpu_base_graph_color: Color,
    pub cpu_info_block_color: Color,
    pub cpu_text_color: Color,

    // for memory
    pub memory_container_selected_color: Color,
    pub memory_main_block_color: Color,
    pub used_memory_base_graph_color: Color,
    pub available_memory_base_graph_color: Color,
    pub free_memory_base_graph_color: Color,
    pub cached_memory_base_graph_color: Color,
    pub swap_memory_base_graph_color: Color,
    pub memory_text_color: Color,

    // for disk
    pub disk_container_selected_color: Color,
    pub disk_main_block_color: Color,
    pub disk_bytes_written_base_graph_color: Color,
    pub disk_bytes_read_base_graph_color: Color,
    pub disk_text_color: Color,

    // for network
    pub network_container_selected_color: Color,
    pub network_main_block_color: Color,
    pub network_received_base_graph_color: Color,
    pub network_transmitted_base_graph_color: Color,
    pub network_info_block_color: Color,
    pub network_text_color: Color,

    // for process
    pub process_container_selected_color: Color,
    pub process_main_block_color: Color,
    pub process_base_graph_color: Color,
    pub process_info_block_color: Color,
    pub process_title_color: Color,
    pub process_text_color: Color,
    pub process_selected_color_bg: Color,
    pub process_selected_color_fg: Color,
}

const MAXIMUM_DATA_COLLECTION: usize = 500;

pub struct CpuData {
    pub id: String,
    pub brand: String,
    pub usage: f32,
    pub usage_history_vec: Vec<f32>,
}

pub struct MemoryData {
    pub total_memory: f64,
    pub available_memory_vec: Vec<f64>, // available is the combination of free memory, cachedmemory and ready to be reused memory
    pub used_memory_vec: Vec<f64>,
    pub used_swap_vec: Vec<f64>,
    pub free_memory_vec: Vec<f64>, // free means memory that is not used at all
    pub cached_memory_vec: Vec<f64>,
}

pub struct DiskData {
    pub name: String,
    pub total_space: f64,
    pub available_space: f64,
    pub used_space: f64,
    pub bytes_written_vec: Vec<f64>, // Number of written bytes since the last refresh. in B
    pub bytes_read_vec: Vec<f64>,    // Number of read bytes since the last refresh. in B
    pub file_system: String, // file system used on this disk (so for example: EXT4, NTFS, etc…).
    pub mount_point: String, // mount point of the disk (/ for example). And mount point will also served as the unique identifier for the disk
    pub disk_kind: String,   // kind of disk.( SSD for example )
    pub is_updated: bool, // this was to keep tracked of exsiting disk data we collected was still connected to the system
}

pub struct NetworkData {
    pub interface_name: String,
    pub ip_network: Option<String>,
    pub current_received_vec: Vec<f64>,
    pub current_transmitted_vec: Vec<f64>,
    pub total_received: f64,
    pub total_transmitted: f64,
    pub is_updated: bool,
}

#[derive(Debug, Clone)]
pub struct ProcessData {
    pub pid: u32,
    pub name: String,
    pub exe_path: Option<String>,
    pub cmd: Vec<String>,
    pub user: String,
    pub cpu_usage: Vec<f32>,
    pub thread_count: u32,
    pub memory: Vec<f64>,
    pub status: String,
    pub elapsed: u64,
    pub parent: String,
    pub current_read_disk_usage: u64,
    pub total_read_disk_usage: u64,
    pub current_write_disk_usage: u64,
    pub total_write_disk_usage: u64,
    pub is_updated: bool,
}

pub struct CurrentProcessSignalStateData {
    pub pid: String,
    pub signal: Option<Signal>,
    pub signal_id: Option<u16>,
    pub name: String,
    pub yes_confirmation: bool,
    pub no_confirmation: bool,
}

impl CpuData {
    pub fn new(id: i8, brand: String, usage: f32) -> CpuData {
        let id = if id == -1 {
            "CPU-AVG".to_string()
        } else {
            format!("CPU{}", id)
        };
        CpuData {
            id,
            brand,
            usage,
            usage_history_vec: vec![],
        }
    }

    pub fn update(&mut self, id: i8, usage: f32) {
        let id = if id == -1 {
            "CPU-AVG".to_string()
        } else {
            format!("CPU{}", id)
        };
        if id == self.id {
            self.usage = usage;
            if self.usage_history_vec.len() >= MAXIMUM_DATA_COLLECTION {
                self.usage_history_vec.remove(0);
            }
            self.usage_history_vec.push(usage);
        }
    }
}

impl MemoryData {
    pub fn default() -> MemoryData {
        MemoryData {
            total_memory: -1.0,
            available_memory_vec: vec![0.0],
            used_memory_vec: vec![0.0],
            used_swap_vec: vec![0.0],
            free_memory_vec: vec![0.0],
            cached_memory_vec: vec![0.0],
        }
    }

    pub fn new(
        total: f64,
        available: f64,
        used: f64,
        used_swap: f64,
        free: f64,
        cached: f64,
    ) -> MemoryData {
        return MemoryData {
            total_memory: total,
            available_memory_vec: vec![available],
            used_memory_vec: vec![used],
            used_swap_vec: vec![used_swap],
            free_memory_vec: vec![free],
            cached_memory_vec: vec![cached],
        };
    }

    pub fn update(
        &mut self,
        total: f64,
        available: f64,
        used: f64,
        used_swap: f64,
        free: f64,
        cached: f64,
    ) {
        self.total_memory = total;
        self.available_memory_vec.push(available);
        self.used_memory_vec.push(used);
        self.used_swap_vec.push(used_swap);
        self.free_memory_vec.push(free);
        self.cached_memory_vec.push(cached);

        if self.available_memory_vec.len() > MAXIMUM_DATA_COLLECTION {
            self.available_memory_vec.remove(0);
        }
        if self.used_memory_vec.len() > MAXIMUM_DATA_COLLECTION {
            self.used_memory_vec.remove(0);
        }
        if self.used_swap_vec.len() > MAXIMUM_DATA_COLLECTION {
            self.used_swap_vec.remove(0);
        }
        if self.free_memory_vec.len() > MAXIMUM_DATA_COLLECTION {
            self.free_memory_vec.remove(0);
        }
        if self.cached_memory_vec.len() > MAXIMUM_DATA_COLLECTION {
            self.cached_memory_vec.remove(0);
        }
    }
}

impl DiskData {
    pub fn new(
        name: String,
        total_space: f64,
        available_space: f64,
        used_space: f64,
        bytes_written: f64,
        bytes_read: f64,
        file_system: String,
        mount_point: String,
        kind: String,
    ) -> DiskData {
        DiskData {
            name,
            total_space,
            available_space,
            used_space,
            bytes_written_vec: vec![bytes_written],
            bytes_read_vec: vec![bytes_read],
            file_system,
            mount_point,
            disk_kind: kind,
            is_updated: true,
        }
    }

    pub fn update(
        &mut self,
        name: String,
        total_space: f64,
        available_space: f64,
        used_space: f64,
        bytes_written: f64,
        bytes_read: f64,
        file_system: String,
        mount_point: String,
        kind: String,
    ) {
        if mount_point == self.mount_point {
            self.name = name;
            self.total_space = total_space;
            self.available_space = available_space;
            self.used_space = used_space;
            self.file_system = file_system;
            self.disk_kind = kind;
            self.bytes_written_vec.push(bytes_written);
            self.bytes_read_vec.push(bytes_read);
            if self.bytes_written_vec.len() > MAXIMUM_DATA_COLLECTION {
                self.bytes_written_vec.remove(0);
            }
            if self.bytes_read_vec.len() > MAXIMUM_DATA_COLLECTION {
                self.bytes_read_vec.remove(0);
            }
            self.is_updated = true;
        }
    }
}

impl NetworkData {
    pub fn new(
        interface_name: String,
        ip_network: Option<String>,
        current_received: f64,
        current_transmitted: f64,
        total_received: f64,
        total_transmitted: f64,
    ) -> NetworkData {
        return NetworkData {
            interface_name,
            ip_network,
            current_received_vec: vec![current_received],
            current_transmitted_vec: vec![current_transmitted],
            total_received,
            total_transmitted,
            is_updated: true,
        };
    }

    pub fn update(
        &mut self,
        interface_name: String,
        ip_network: Option<String>,
        current_received: f64,
        current_transmitted: f64,
        total_received: f64,
        total_transmitted: f64,
    ) {
        self.interface_name = interface_name;
        self.ip_network = ip_network;
        self.current_received_vec.push(current_received);
        self.current_transmitted_vec.push(current_transmitted);
        if self.current_received_vec.len() > MAXIMUM_DATA_COLLECTION {
            self.current_received_vec.remove(0);
        }
        if self.current_transmitted_vec.len() > MAXIMUM_DATA_COLLECTION {
            self.current_transmitted_vec.remove(0);
        }
        self.total_received = total_received;
        self.total_transmitted = total_transmitted;
        self.is_updated = true;
    }
}

impl ProcessData {
    pub fn new(
        pid: u32,
        name: String,
        exe_path: Option<String>,
        cmd: Vec<String>,
        user: String,
        cpu_usage: f32,
        thread_count: u32,
        memory: f64,
        status: String,
        elapsed: u64,
        parent: String,
        current_read_disk_usage: u64,
        total_read_disk_usage: u64,
        current_write_disk_usage: u64,
        total_write_disk_usage: u64,
    ) -> ProcessData {
        return ProcessData {
            pid,
            name,
            exe_path,
            cmd,
            user,
            cpu_usage: vec![cpu_usage],
            thread_count,
            memory: vec![memory],
            status,
            elapsed,
            parent,
            is_updated: true,
            current_read_disk_usage,
            total_read_disk_usage,
            current_write_disk_usage,
            total_write_disk_usage,
        };
    }

    pub fn update(
        &mut self,
        pid: u32,
        name: String,
        exe_path: Option<String>,
        cmd: Vec<String>,
        user: String,
        cpu_usage: f32,
        thread_count: u32,
        memory: f64,
        status: String,
        elapsed: u64,
        parent: String,
        current_read_disk_usage: u64,
        total_read_disk_usage: u64,
        current_write_disk_usage: u64,
        total_write_disk_usage: u64,
    ) {
        if self.pid == pid {
            self.name = name;
            self.exe_path = exe_path;
            self.cmd = cmd;
            self.user = user;
            self.cpu_usage.push(cpu_usage);
            self.thread_count = thread_count;
            self.memory.push(memory);
            self.status = status;
            self.elapsed = elapsed;
            self.parent = parent;
            self.current_read_disk_usage = current_read_disk_usage;
            self.total_read_disk_usage = total_read_disk_usage;
            self.current_write_disk_usage = current_write_disk_usage;
            self.total_write_disk_usage = total_write_disk_usage;

            if self.cpu_usage.len() > MAXIMUM_DATA_COLLECTION {
                self.cpu_usage.remove(0);
            }

            if self.memory.len() > MAXIMUM_DATA_COLLECTION {
                self.memory.remove(0);
            }
            self.is_updated = true;
        }
    }
}

// the structure of info collected from a seperated thread
// a C infront mean Collected
pub struct CSysInfo {
    pub cpus: Vec<CCpuData>,
    pub memory: CMemoryData,
    pub disks: Vec<CDiskData>,
    pub networks: Vec<CNetworkData>,
}

pub struct CProcessesInfo {
    pub processes: Vec<CProcessData>,
}

pub struct CCpuData {
    pub id: i8,
    pub brand: String,
    pub usage: f32,
}

pub struct CMemoryData {
    pub total_memory: f64,
    pub available_memory: f64, // available is the combination of free memory, cached memory and ready to be reused memory
    pub used_memory: f64,
    pub used_swap: f64,
    pub free_memory: f64, // free means memory that is not used at all
    pub cached_memory: f64,
}

pub struct CDiskData {
    pub name: String,
    pub total_space: f64,
    pub available_space: f64,
    pub used_space: f64,
    pub bytes_written: f64, // Number of written bytes since the last refresh. Will be return in KiB
    pub bytes_read: f64,    // Number of read bytes since the last refresh. Will be return in KiB
    pub file_system: String, // file system used on this disk (so for example: EXT4, NTFS, etc…).
    pub mount_point: String, // mount point of the disk (/ for example).
    pub kind: String,       // kind of disk.( SSD for example )
}

pub struct CNetworkData {
    pub interface_name: String,
    pub ip_network: Option<String>,
    pub current_received: f64,
    pub total_received: f64,
    pub current_transmitted: f64,
    pub total_transmitted: f64,
}

pub struct CProcessData {
    pub pid: u32,
    pub name: String,
    pub exe_path: Option<String>,
    pub cmd: Vec<String>,
    pub user: String,
    pub cpu_usage: f32,
    pub thread_count: u32,
    pub memory: f64,
    pub status: String,
    pub elapsed: u64,
    pub parent: String,
    pub current_read_disk_usage: u64,
    pub total_read_disk_usage: u64,
    pub current_write_disk_usage: u64,
    pub total_write_disk_usage: u64,
}

#[derive(PartialEq)]
pub enum SelectedContainer {
    Cpu,
    Memory,
    Disk,
    Network,
    Process,
    Menu,
    None,
}

#[derive(PartialEq)]
pub enum AppState {
    View,
    Typing, // mainly to indicated that user is typing ( will be used for process filtering )
    Menu,
    Popup,
}

#[derive(PartialEq)]
pub enum AppPopUpType {
    None,
    KillConfirmation,
    TerminateConfirmation,
    SignalMenu,
}

#[derive(PartialEq, Clone)]
pub enum ProcessSortType {
    Thread,
    Memory,
    Cpu,
    Pid,
    Name,
    Command,
    User,
}

impl ProcessSortType {
    pub fn get_process_sort_type_from_int(id: u8) -> ProcessSortType {
        match id {
            0 => ProcessSortType::Thread,
            1 => ProcessSortType::Memory,
            2 => ProcessSortType::Cpu,
            3 => ProcessSortType::Pid,
            4 => ProcessSortType::Name,
            5 => ProcessSortType::Command,
            6 => ProcessSortType::User,
            _ => ProcessSortType::Thread,
        }
    }

    pub fn get_sort_string_name(&self) -> String {
        match self {
            ProcessSortType::Thread => "Thread".to_string(),
            ProcessSortType::Memory => "Memory".to_string(),
            ProcessSortType::Cpu => "CPU".to_string(),
            ProcessSortType::Pid => "PID".to_string(),
            ProcessSortType::Name => "Name".to_string(),
            ProcessSortType::Command => "Command".to_string(),
            ProcessSortType::User => "User".to_string(),
        }
    }

    pub fn total_selection_count() -> u8 {
        7
    }
}

impl AppPopUpType {
    pub fn get_string_name(&self) -> String {
        match self {
            AppPopUpType::KillConfirmation => " KILL ".to_string(),
            AppPopUpType::TerminateConfirmation => " TERMINATION ".to_string(),
            AppPopUpType::SignalMenu => " SIGNAL ".to_string(),
            _ => "".to_string(),
        }
    }
}

pub trait SignalExt {
    fn get_display_name(&self) -> String;
}

impl SignalExt for Signal {
    fn get_display_name(&self) -> String {
        match self {
            Signal::Hangup => "SIGHUP".to_string(),
            Signal::Interrupt => "SIGINT".to_string(),
            Signal::Quit => "SIGQUIT".to_string(),
            Signal::Illegal => "SIGILL".to_string(),
            Signal::Trap => "SIGTRAP".to_string(),
            Signal::Abort => "SIGABRT".to_string(),
            Signal::IOT => "SIGIOT".to_string(),
            Signal::Bus => "SIGBUS".to_string(),
            Signal::FloatingPointException => "SIGFPE".to_string(),
            Signal::Kill => "SIGKILL".to_string(),
            Signal::User1 => "SIGUSR1".to_string(),
            Signal::Segv => "SIGSEGV".to_string(),
            Signal::User2 => "SIGUSR2".to_string(),
            Signal::Pipe => "SIGPIPE".to_string(),
            Signal::Alarm => "SIGALRM".to_string(),
            Signal::Term => "SIGTERM".to_string(),
            Signal::Child => "SIGCHLD".to_string(),
            Signal::Continue => "SIGCONT".to_string(),
            Signal::Stop => "SIGSTOP".to_string(),
            Signal::TSTP => "SIGTSTP".to_string(),
            Signal::TTIN => "SIGTTIN".to_string(),
            Signal::TTOU => "SIGTTOU".to_string(),
            Signal::Urgent => "SIGURG".to_string(),
            Signal::XCPU => "SIGXCPU".to_string(),
            Signal::XFSZ => "SIGXFSZ".to_string(),
            Signal::VirtualAlarm => "SIGVTALRM".to_string(),
            Signal::Profiling => "SIGPROF".to_string(),
            Signal::Winch => "SIGWINCH".to_string(),
            Signal::IO => "SIGIO".to_string(),
            Signal::Poll => "SIGPOLL".to_string(),
            Signal::Sys => "SIGSYS".to_string(),
            _ => "".to_string(),
        }
    }
}
