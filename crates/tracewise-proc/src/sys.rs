//! Operating system process inspection routines delegated to `tracewise-platform`.
pub use tracewise_platform::process::{
    chain, exe, get_ppid, get_proc_args, get_proc_name, get_proc_path, get_process_entry, is_alive, name, parent, parent_pid, process_chain,
};
