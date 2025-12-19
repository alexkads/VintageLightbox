slint::include_modules!();


use infrastructure::{
    create_pool, run_migrations,
    PhotoRepositoryImpl, ExifReader,
    ThumbnailGeneratorImpl,
};
    use use_cases::{ImportPhotoUseCase, SavePhotoEditsUseCase};
    use adapters::controllers::{ImportController, EditorController};
    use adapters::view_models::PhotoViewModel;
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
        let save_photo_edits_use_case = Arc::new(SavePhotoEditsUseCase::new(
            photo_repository.clone()
        ));
    
        // 3. Setup Controllers
        let import_controller = Arc::new(ImportController::new(import_photo_use_case.clone()));
        let library_controller = Arc::new(adapters::controllers::LibraryController::new(photo_repository.clone()));
        let editor_controller = Arc::new(EditorController::new(save_photo_edits_use_case.clone()));

    // 4. Setup UI
    let main_window = MainWindow::new()?;
    let main_window_weak = main_window.as_weak();

    // State to hold photos for detail view lookup
    let photos_state = Arc::new(std::sync::Mutex::new(Vec::<PhotoViewModel>::new()));

    // Initial load of photos
    
    // --- Image Processing State ---
    use std::sync::{Arc, Mutex};
    // Active full-res image (loaded in memory for editing)
    let active_image: Arc<Mutex<Option<image::DynamicImage>>> = Arc::new(Mutex::new(None));
    
    // Helper to process image
    fn process_image(img: &image::DynamicImage, exposure: f32, contrast: f32) -> slint::Image {
        // 1. Exposure (Brighten)
        // exposure is -5.0 to 5.0. brighten takes i32. 
        // We'll approximate exposure by simple brightening.
        // A better value would be scaling pixel values, but 'brighten' is available in image::imageops.
        // brighten(img, value): value is i32.
        let brightened = if exposure != 0.0 {
            // Mapping -5.0..5.0 to roughly -50..50 for i32 brighten
            image::imageops::brighten(img, (exposure * 10.0) as i32)
        } else {
            img.to_rgba8()
        };

        // 2. Contrast
        // adjust_contrast(img, c): c is f32. 
        // 1.0 = original. < 1.0 low contrast, > 1.0 high contrast.
        let contrasted = if contrast != 1.0 {
            image::imageops::contrast(&brightened, contrast)
        } else {
            brightened
        };

        // Convert back to Slint Image
        let buffer = contrasted;
        let pixel_buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            buffer.as_raw(),
            buffer.width(),
            buffer.height(),
        );
        slint::Image::from_rgba8(pixel_buffer)
    }

    let active_image_for_nav = active_image.clone();

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
    let active_image_for_tile = active_image.clone(); // Added clone
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

                // Extract edits before dropping lock
                let edit_exposure_val = photo.edit_exposure.unwrap_or(0.0);
                let edit_contrast_val = photo.edit_contrast.unwrap_or(1.0);

                // Drop lock before loading image
                drop(photos);

                let image_path = Path::new(&path_str);
                
                // Load using image crate for editing support
                let image = match image::open(image_path) {
                    Ok(dyn_img) => {
                         // Save to active_image state
                         if let Ok(mut active) = active_image_for_tile.lock() {
                             *active = Some(dyn_img.clone());
                         }
                         
                         // Apply edits if they exist
                         process_image(&dyn_img, edit_exposure_val, edit_contrast_val)
                    },
                    Err(_) => {
                         // Fallback to thumbnail or default
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
                    
                    // Initialize edit sliders
                    ui.set_active_exposure(edit_exposure_val);
                    ui.set_active_contrast(edit_contrast_val);
                    
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


    
    // --- Callbacks ---

    // Updated Navigation Callback (loads active_image)
    let photos_state_for_nav = photos_state.clone();
    let main_window_weak_for_nav = main_window.as_weak();
    
    main_window.on_navigate(move |direction| {
        // ... (existing navigation logic to find target_photo) ...
        // Re-implementing logic to include saving to active_image
        let mut target_photo: Option<PhotoViewModel> = None;

        if let Ok(photos) = photos_state_for_nav.lock() {
             if let Some(current_ui) = main_window_weak_for_nav.upgrade() {
                 let current_id = current_ui.get_detail_id().as_str().to_string();
                 if let Some(pos) = photos.iter().position(|p| p.id == current_id) {
                     let new_pos = if direction > 0 { pos + 1 } else { if pos > 0 { pos - 1 } else { 0 } };
                     if new_pos < photos.len() {
                         target_photo = Some(photos[new_pos].clone()); // Clone minimal data
                     }
                 }
            }
        }
        
        if let Some(photo) = target_photo {
             let path_str = photo.path.clone();
             let id = photo.id.clone();
             // Metadata update
             if let Some(ui) = main_window_weak_for_nav.upgrade() {
                ui.set_detail_id(slint::SharedString::from(id));
                ui.set_detail_name(slint::SharedString::from(photo.name));
                ui.set_detail_date(slint::SharedString::from(photo.date));
                ui.set_detail_camera(slint::SharedString::from(photo.camera));
                ui.set_detail_exposure(slint::SharedString::from(photo.exposure));
                ui.set_detail_rating(slint::SharedString::from(format!("Rating: {}/5", photo.rating)));
                
                // Initialize edit sliders
                let edit_exposure = photo.edit_exposure.unwrap_or(0.0);
                let edit_contrast = photo.edit_contrast.unwrap_or(1.0);
                ui.set_active_exposure(edit_exposure);
                ui.set_active_contrast(edit_contrast);
             }

             // Load Image
             let image_path = Path::new(&path_str);
             // Use image crate to load DynamicImage for processing
             match image::open(image_path) {
                 Ok(dyn_img) => {
                     // Save to active_image state
                     if let Ok(mut active) = active_image_for_nav.lock() {
                         *active = Some(dyn_img.clone());
                     }
                     // Apply edits if they exist
                     let edit_exposure = photo.edit_exposure.unwrap_or(0.0);
                     let edit_contrast = photo.edit_contrast.unwrap_or(1.0);
                     
                     let slint_img = process_image(&dyn_img, edit_exposure, edit_contrast);
                     
                     if let Some(ui) = main_window_weak_for_nav.upgrade() {
                         ui.set_detail_image(slint_img);
                     }
                 },
                 Err(e) => {
                     println!("Error loading image for editing: {:?}", e);
                     // Fallback to thumbnail or default if load fails
                     if let Some(ui) = main_window_weak_for_nav.upgrade() {
                        ui.set_detail_image(slint::Image::default()); 
                     }
                 }
             }
        }
    });

    // Save Edits Callback
    let editor_controller_clone = editor_controller.clone();
    main_window.on_save_edits(move |id, exposure, contrast| {
        let controller = editor_controller_clone.clone();
        let id_str = id.as_str().to_string();
        
        tokio::spawn(async move {
            match controller.save_edits(id_str, exposure, contrast).await {
                Ok(_) => println!("Edits saved successfully!"),
                Err(e) => eprintln!("Failed to save edits: {}", e),
            }
        });
    });

    // Apply Edits Callback
    let active_image_for_edits = active_image.clone();
    let main_window_weak_for_edits = main_window.as_weak();
    main_window.on_apply_edits(move |exposure, contrast| {
        // Debounce or optimize? For now, block main thread (MVP)
        let processed_img = if let Ok(active_opt) = active_image_for_edits.lock() {
            if let Some(img) = active_opt.as_ref() {
                Some(process_image(img, exposure, contrast))
            } else {
                None
            }
        } else {
            None
        };

        if let Some(img) = processed_img {
            if let Some(ui) = main_window_weak_for_edits.upgrade() {
                ui.set_detail_image(img);
            }
        }
    });

    // Update tile_click to also load active_image
    // ... (This requires updating on_tile_clicked logic similarly to on_navigate)

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
