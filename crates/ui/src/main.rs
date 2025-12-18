slint::include_modules!();

use std::sync::Arc;
use infrastructure::{
    create_pool, run_migrations,
    PhotoRepositoryImpl, ExifReader,
};
use use_cases::ImportPhotoUseCase;
use adapters::controllers::ImportController;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Infrastructure
    // Usar arquivo local para persistência
    let database_url = "sqlite:vintage_lightbox.db?mode=rwc"; 
    let pool = create_pool(database_url).await?;
    
    // Executar migrations ao iniciar
    run_migrations(&pool).await?;

    let photo_repository = Arc::new(PhotoRepositoryImpl::new(pool));
    let metadata_extractor = Arc::new(ExifReader);

    // 2. Setup Use Cases
    let import_photo_use_case = Arc::new(ImportPhotoUseCase::new(
        photo_repository, 
        metadata_extractor
    ));

    // 3. Setup Controllers
    let import_controller = Arc::new(ImportController::new(import_photo_use_case));

    // 4. Setup UI
    let main_window = MainWindow::new()?;
    let _main_window_weak = main_window.as_weak();

    let controller = import_controller.clone();

    main_window.on_import_clicked(move || {
        let controller = controller.clone();
        
        // Spawn async task para não bloquear a UI thread
        tokio::spawn(async move {
            println!("Opening file dialog...");
            
            // rfd AsyncFileDialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter("Images", &["jpg", "png", "raw", "cr2", "nef"])
                .pick_file()
                .await;

            if let Some(file_handle) = file {
                // Obter caminho como string
                // Nota: path() retorna PathBuf. to_utf8 flossy conversion.
                #[cfg(not(target_arch = "wasm32"))]
                let path_str = file_handle.path().to_string_lossy().to_string();
                
                println!("Selected file: {}", path_str);
                
                // Chamar controller
                match controller.import_files(vec![path_str]).await {
                    Ok(_) => println!("Import successful! Check database."),
                    Err(e) => eprintln!("Import failed: {}", e),
                }
            } else {
                println!("No file selected.");
            }
        });
    });

    println!("Starting VintageLightbox UI...");
    main_window.run()?;
    
    Ok(())
}
