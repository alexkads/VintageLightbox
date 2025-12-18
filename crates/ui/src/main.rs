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
        photo_repository.clone(), 
        metadata_extractor
    ));

    // 3. Setup Controllers
    let import_controller = Arc::new(ImportController::new(import_photo_use_case.clone()));
    let library_controller = Arc::new(adapters::controllers::LibraryController::new(photo_repository.clone()));

    // 4. Setup UI
    let main_window = MainWindow::new()?;
    let main_window_weak = main_window.as_weak();

    // Initial load of photos
    {
        let library_controller = library_controller.clone();
        let main_window_weak = main_window_weak.clone();

        tokio::spawn(async move {
            match library_controller.get_all_photos().await {
                Ok(view_models) => {
                     let _ = slint::invoke_from_event_loop(move || {
                        if let Some(main_window) = main_window_weak.upgrade() {
                            let tile_data: Vec<TileData> = view_models.into_iter().map(|vm| {
                                TileData {
                                    name: slint::SharedString::from(vm.name),
                                    // Placeholder image for now, real thumbnail loading needed later
                                    // Slint image loading from path is synchronous and tricky without custom loader
                                    // For POC we might leave image empty or use a resource
                                    image: slint::Image::default(), 
                                }
                            }).collect();
                            
                            let model = std::rc::Rc::new(slint::VecModel::from(tile_data));
                            main_window.set_photo_model(slint::ModelRc::from(model));
                        }
                    });
                }
                Err(e) => eprintln!("Failed to load photos: {}", e),
            }
        });
    }

    let controller = import_controller.clone();
    let library_controller_for_import = library_controller.clone();
    let main_window_weak_for_import = main_window.as_weak();

    main_window.on_import_clicked(move || {
        let controller = controller.clone();
        let library_controller = library_controller_for_import.clone();
        let main_window_weak = main_window_weak_for_import.clone();
        
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
                    Ok(_) => {
                        println!("Import successful! Refreshing library...");
                        // Refresh library
                         match library_controller.get_all_photos().await {
                            Ok(view_models) => {
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(main_window) = main_window_weak.upgrade() {
                                        let tile_data: Vec<TileData> = view_models.into_iter().map(|vm| {
                                            TileData {
                                                name: slint::SharedString::from(vm.name),
                                                image: slint::Image::default(), 
                                            }
                                        }).collect();
                                        
                                        let model = std::rc::Rc::new(slint::VecModel::from(tile_data));
                                        main_window.set_photo_model(slint::ModelRc::from(model));
                                    }
                                });
                            }
                            Err(e) => eprintln!("Failed to refresh photos: {}", e),
                        }
                    },
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
