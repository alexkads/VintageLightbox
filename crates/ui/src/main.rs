slint::include_modules!();

use std::sync::Arc;
use infrastructure::{
    create_pool, run_migrations,
    PhotoRepositoryImpl, ExifReader,
    ThumbnailGeneratorImpl,
};
use use_cases::ImportPhotoUseCase;
use adapters::controllers::ImportController;
use std::path::Path;

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
    let thumbnail_generator = Arc::new(ThumbnailGeneratorImpl::new());

    // 2. Setup Use Cases
    let import_photo_use_case = Arc::new(ImportPhotoUseCase::new(
        photo_repository.clone(), 
        metadata_extractor,
        thumbnail_generator
    ));

    // 3. Setup Controllers
    let import_controller = Arc::new(ImportController::new(import_photo_use_case.clone()));
    let library_controller = Arc::new(adapters::controllers::LibraryController::new(photo_repository.clone()));

    // 4. Setup UI
    let main_window = MainWindow::new()?;
    let main_window_weak = main_window.as_weak();

    // State to hold photos for detail view lookup
    let photos_state = Arc::new(std::sync::Mutex::new(Vec::<adapters::view_models::PhotoViewModel>::new()));

    // Initial load of photos
    {
        let library_controller = library_controller.clone();
        let main_window_weak = main_window_weak.clone();
        let photos_state = photos_state.clone();

        tokio::spawn(async move {
            match library_controller.get_all_photos().await {
                Ok(view_models) => {
                     // Update state
                     if let Ok(mut photos) = photos_state.lock() {
                         *photos = view_models.clone();
                     }

                     let _ = slint::invoke_from_event_loop(move || {
                        if let Some(main_window) = main_window_weak.upgrade() {
                            let rows: Vec<RowData> = view_models.chunks(5).map(|chunk| {
                                let tiles: Vec<TileData> = chunk.iter().map(|vm| {
                                    let image = if let Some(path) = &vm.thumbnail_path {
                                        slint::Image::load_from_path(Path::new(path)).unwrap_or_default()
                                    } else {
                                        slint::Image::default()
                                    };
                                    
                                    TileData {
                                        id: slint::SharedString::from(&vm.id),
                                        name: slint::SharedString::from(&vm.name),
                                        image,
                                    }
                                }).collect();
                                
                                RowData {
                                    tiles: slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(tiles)))
                                }
                            }).collect();
                            
                            let model = std::rc::Rc::new(slint::VecModel::from(rows));
                            main_window.set_grid_model(slint::ModelRc::from(model));
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
    let photos_state_for_import = photos_state.clone();

    main_window.on_import_clicked(move || {
        let controller = controller.clone();
        let library_controller = library_controller_for_import.clone();
        let main_window_weak = main_window_weak_for_import.clone();
        let photos_state = photos_state_for_import.clone();
        
        tokio::spawn(async move {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("Images", &["jpg", "png", "raw", "cr2", "nef"])
                .pick_file()
                .await;

            if let Some(file_handle) = file {
                #[cfg(not(target_arch = "wasm32"))]
                let path_str = file_handle.path().to_string_lossy().to_string();
                
                match controller.import_files(vec![path_str]).await {
                    Ok(_) => {
                        match library_controller.get_all_photos().await {
                            Ok(view_models) => {
                                if let Ok(mut photos) = photos_state.lock() {
                                    *photos = view_models.clone();
                                }

                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(main_window) = main_window_weak.upgrade() {
                                        let rows: Vec<RowData> = view_models.chunks(5).map(|chunk| {
                                            let tiles: Vec<TileData> = chunk.iter().map(|vm| {
                                                let image = if let Some(path) = &vm.thumbnail_path {
                                                    slint::Image::load_from_path(Path::new(path)).unwrap_or_default()
                                                } else {
                                                    slint::Image::default()
                                                };
                                                
                                                TileData {
                                                    id: slint::SharedString::from(&vm.id),
                                                    name: slint::SharedString::from(&vm.name),
                                                    image,
                                                }
                                            }).collect();
                                            
                                            RowData {
                                                tiles: slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(tiles)))
                                            }
                                        }).collect();
                                        
                                        let model = std::rc::Rc::new(slint::VecModel::from(rows));
                                        main_window.set_grid_model(slint::ModelRc::from(model));
                                    }
                                });
                            }
                            Err(e) => eprintln!("Failed to refresh photos: {}", e),
                        }
                    },
                    Err(e) => eprintln!("Import failed: {}", e),
                }
            }
        });
    });

    let main_window_weak_for_tile = main_window.as_weak();
    let photos_state_for_tile = photos_state.clone();
    main_window.on_tile_clicked(move |id_str| {
        let id_string = id_str.as_str().to_string();
        if let Ok(photos) = photos_state_for_tile.lock() {
            if let Some(photo) = photos.iter().find(|p| p.id == id_string) {
                let path_str = photo.path.clone();
                let thumbnail_path_str = photo.thumbnail_path.clone();
                
                let id = photo.id.clone();
                // Metadata
                let name = slint::SharedString::from(&photo.name);
                let date = slint::SharedString::from(&photo.date);
                let camera = slint::SharedString::from(&photo.camera);
                let exposure = slint::SharedString::from(&photo.exposure);
                let rating = slint::SharedString::from(format!("Rating: {}/5", photo.rating));

                // Drop lock before loading image
                drop(photos);

                let image_path = Path::new(&path_str);
                
                let image = match slint::Image::load_from_path(image_path) {
                    Ok(img) => img,
                    Err(_) => {
                         if let Some(thumb) = &thumbnail_path_str {
                            slint::Image::load_from_path(Path::new(thumb)).unwrap_or_default()
                        } else {
                            slint::Image::default()
                        }
                    }
                };
                
                if let Some(ui) = main_window_weak_for_tile.upgrade() {
                    ui.set_detail_id(slint::SharedString::from(id)); // Use cloned ID
                    ui.set_detail_image(image);
                    ui.set_detail_name(name);
                    ui.set_detail_date(date);
                    ui.set_detail_camera(camera);
                    ui.set_detail_exposure(exposure);
                    ui.set_detail_rating(rating);
                    ui.set_current_view(1);
                }
            }
        }
    });

    // Rating Callback
    let photos_state_for_rate = photos_state.clone();
    let main_window_weak_for_rate = main_window.as_weak();
    // Assuming we would inject a RatePhotoUseCase here in real imp.
    main_window.on_rate_photo(move |id, rating| {
        println!("Rate photo: id={} rating={}", id, rating);
        // Minimal state update to reflect change immediately in UI (optimistic)
        let id_string = id.as_str().to_string();
        if let Some(ui) = main_window_weak_for_rate.upgrade() {
             ui.set_detail_rating(slint::SharedString::from(format!("Rating: {}/5", rating)));
        }
        
        // Update in-memory state
        if let Ok(mut photos) = photos_state_for_rate.lock() {
            if let Some(photo) = photos.iter_mut().find(|p| p.id == id_string) {
                photo.rating = rating;
            }
        }
        // TODO: Call Controller -> UseCase -> Repository to persist
    });

    // Navigation Callback
    let photos_state_for_nav = photos_state.clone();
    let main_window_weak_for_nav = main_window.as_weak();
    main_window.on_navigate(move |direction| {
        let mut target_index: Option<usize> = None;
        let mut target_photo: Option<adapters::view_models::PhotoViewModel> = None;

        if let Ok(photos) = photos_state_for_nav.lock() {
            if let Some(current_ui) = main_window_weak_for_nav.upgrade() {
                 let current_id = current_ui.get_detail_id().as_str().to_string();
                 if let Some(pos) = photos.iter().position(|p| p.id == current_id) {
                     let new_pos = if direction > 0 {
                         (pos + 1)
                     } else {
                         if pos > 0 { pos - 1 } else { 0 }
                     };
                     
                     if new_pos < photos.len() {
                         target_index = Some(new_pos);
                         target_photo = Some(photos[new_pos].clone());
                     }
                 }
            }
        }
        
        if let Some(photo) = target_photo {
             let path_str = photo.path.clone();
             let thumbnail_path_str = photo.thumbnail_path.clone();
             let id = photo.id.clone();
             
             // Load image outside lock (implied here as we cloned target_photo)
             let image_path = Path::new(&path_str);
             let image = match slint::Image::load_from_path(image_path) {
                Ok(img) => img,
                Err(_) => {
                     if let Some(thumb) = &thumbnail_path_str {
                        slint::Image::load_from_path(Path::new(thumb)).unwrap_or_default()
                    } else {
                        slint::Image::default()
                    }
                }
            };
            
            if let Some(ui) = main_window_weak_for_nav.upgrade() {
                ui.set_detail_id(slint::SharedString::from(id));
                ui.set_detail_image(image);
                ui.set_detail_name(slint::SharedString::from(photo.name));
                ui.set_detail_date(slint::SharedString::from(photo.date));
                ui.set_detail_camera(slint::SharedString::from(photo.camera));
                ui.set_detail_exposure(slint::SharedString::from(photo.exposure));
                ui.set_detail_rating(slint::SharedString::from(format!("Rating: {}/5", photo.rating)));
            }
        }
    });

    let main_window_weak_for_back = main_window.as_weak();
    main_window.on_back_clicked(move || {
        if let Some(ui) = main_window_weak_for_back.upgrade() {
            ui.set_current_view(0); // Switch to Library view
            ui.set_detail_image(slint::Image::default()); // Clear memory
        }
    });

    println!("Starting VintageLightbox UI...");
    main_window.run()?;
    
    Ok(())
}
