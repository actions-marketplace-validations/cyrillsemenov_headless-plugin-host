#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub mod linux;
