pub mod history_repo;
pub mod repository;
use domain::import_source::ImportSource;
use sysinfo::Disks;

pub struct DeviceService;

impl DeviceService {
    pub fn new() -> Self {
        Self
    }

    pub fn get_mounted_devices(&self) -> Vec<ImportSource> {
        let disks = Disks::new_with_refreshed_list();
        let mut devices = Vec::new();

        for disk in &disks {
            // Filter logic: we want removable drives mostly, but on Mac many things show as fixed.
            // We'll list everything that is mounted at /Volumes/ (common on Mac for external drives)
            // or is marked as removable.
            
            let mount_point = disk.mount_point();
            let is_in_volumes = mount_point.to_string_lossy().starts_with("/Volumes/");
            
            if disk.is_removable() || is_in_volumes {
                let name = if let Some(n) = disk.name().to_str() {
                    if n.is_empty() {
                        // Fallback to folder name
                        mount_point.file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or("Untitled")
                            .to_string()
                    } else {
                        n.to_string()
                    }
                } else {
                    "Unknown Device".to_string()
                };

                devices.push(ImportSource::new_device(name, mount_point.to_path_buf()));
            }
        }
        
        devices
    }
}
