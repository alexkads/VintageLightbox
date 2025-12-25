use sysinfo::Disks;

fn main() {
    let disks = Disks::new_with_refreshed_list();
    println!("Disks:");
    for disk in &disks {
        println!("  Name: {:?}", disk.name());
        println!("  Mount Point: {:?}", disk.mount_point());
        println!("  Kind: {:?}", disk.kind());
        println!("  Removable: {:?}", disk.is_removable());
        println!("  Total Space: {} bytes", disk.total_space());
        println!("  Available Space: {} bytes", disk.available_space());
        println!("---");
    }
}
