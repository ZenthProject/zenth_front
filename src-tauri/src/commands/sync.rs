#[tauri::command]
pub fn synchronize_accounts() -> Result<(), String> {
    println!("Synchronizing accounts...");
    Ok(())
}