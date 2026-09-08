use domain::services::ThumbnailGenerator;
use domain::value_objects::FilePath;
use infrastructure::thumbnail_generator::ThumbnailGeneratorImpl;
use std::time::Instant;

#[tokio::test]
async fn benchmark_raw_thumbnail_generation() {
    // This benchmark requires a real RAW file to be meaningful.
    // Ensure you have a 'test_assets/sample.nef' or similar.
    // For now we will try to find a sample file or skip.

    let home = std::env::var("HOME").unwrap();
    let sample_path = format!("{}/Pictures/VintageLightbox_Benchmark/sample.NEF", home);

    if !std::path::Path::new(&sample_path).exists() {
        println!("Skipping benchmark: {} not found", sample_path);
        return;
    }

    let generator = ThumbnailGeneratorImpl::new();
    let path = FilePath::new(&sample_path).unwrap();

    println!("Benchmarking generate_set (New Optimized Path)...");
    let start = Instant::now();
    let _ = generator.generate_set(&path, &[300, 2560]).await.unwrap();
    let duration_new = start.elapsed();
    println!("generate_set took: {:?}", duration_new);

    println!("Benchmarking individual generate calls (Legacy Path)...");
    let start = Instant::now();
    let _ = generator.generate(&path, 300).await.unwrap();
    let _ = generator.generate(&path, 2560).await.unwrap();
    let duration_old = start.elapsed();
    println!("2x generate took: {:?}", duration_old);

    println!(
        "Speedup: {:.2}x",
        duration_old.as_secs_f64() / duration_new.as_secs_f64()
    );
}
