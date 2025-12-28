//! ONNX Inpainter Implementation
//!
//! Implementação do IntelligentFillService usando ONNX Runtime
//! para inferência do modelo de inpainting.

use domain::services::intelligent_fill::{
    IntelligentFillParams, IntelligentFillResult, IntelligentFillService, ModelStatus,
};
use domain::DomainResult;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use super::model_loader::ModelLoader;

/// Implementação do serviço de inpainting usando ONNX Runtime
pub struct OnnxInpainterImpl {
    model_loader: ModelLoader,
    status: Arc<Mutex<ModelStatus>>,
    // Futuro: session ONNX será armazenada aqui
    // session: Arc<Mutex<Option<ort::Session>>>,
}

impl OnnxInpainterImpl {
    /// Cria uma nova instância do inpainter
    pub fn new() -> Self {
        Self {
            model_loader: ModelLoader::new(),
            status: Arc::new(Mutex::new(ModelStatus::NotLoaded)),
        }
    }

    /// Cria uma nova instância com ModelLoader personalizado
    pub fn with_model_loader(model_loader: ModelLoader) -> Self {
        Self {
            model_loader,
            status: Arc::new(Mutex::new(ModelStatus::NotLoaded)),
        }
    }

    /// Inicializa o modelo de forma assíncrona
    pub async fn initialize(&self) -> Result<(), String> {
        // Atualizar status para Loading
        {
            let mut status = self.status.lock().unwrap();
            *status = ModelStatus::Loading;
        }

        // Tentar carregar o modelo
        match self.model_loader.ensure_model().await {
            Ok(model_info) => {
                // TODO: Quando ort estiver disponível, carregar sessão ONNX aqui
                // let session = ort::SessionBuilder::new()?
                //     .with_intra_threads(4)?
                //     .commit_from_file(&model_info.path)?;

                // Modelo carregado com sucesso
                #[cfg(debug_assertions)]
                eprintln!(
                    "Modelo de inpainting carregado: {} v{} (input: {}x{})",
                    model_info.name,
                    model_info.version,
                    model_info.input_size,
                    model_info.input_size
                );

                let mut status = self.status.lock().unwrap();
                *status = ModelStatus::Ready;
                Ok(())
            }
            Err(e) => {
                let mut status = self.status.lock().unwrap();
                *status = ModelStatus::Failed(e.clone());
                Err(e)
            }
        }
    }

    /// Gera máscara de rotação para a imagem
    ///
    /// Retorna uma máscara binária onde 1 = área que precisa de fill
    fn generate_rotation_mask(
        &self,
        width: u32,
        height: u32,
        params: &IntelligentFillParams,
    ) -> Vec<u8> {
        let angle_rad = params.crop_settings.angle().to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        let mut mask = vec![0u8; (width * height) as usize];

        let center_x = width as f32 / 2.0;
        let center_y = height as f32 / 2.0;

        for y in 0..height {
            for x in 0..width {
                // Normalizar coordenadas para [-0.5, 0.5]
                let nx = (x as f32 - center_x) / width as f32;
                let ny = (y as f32 - center_y) / height as f32;

                // Rotação inversa para encontrar posição fonte
                let src_x = nx * cos_a + ny * sin_a;
                let src_y = -nx * sin_a + ny * cos_a;

                // Verificar se está fora dos bounds [−0.5, 0.5]
                let is_outside = src_x.abs() > 0.5 || src_y.abs() > 0.5;

                if is_outside {
                    mask[(y * width + x) as usize] = 255;
                }
            }
        }

        mask
    }

    /// Aplica inpainting simples usando extrapolação de bordas (fallback)
    ///
    /// Esta é uma implementação simplificada para quando o modelo ONNX
    /// não está disponível. Usa extrapolação de pixels de borda.
    fn apply_edge_extrapolation(
        &self,
        image_data: &[u8],
        mask: &[u8],
        width: u32,
        height: u32,
    ) -> Vec<u8> {
        let mut result = image_data.to_vec();
        let _stride = (width * 4) as usize; // RGBA

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if mask[idx] > 0 {
                    // Pixel precisa ser preenchido - usar extrapolação de bordas
                    let pixel_idx = idx * 4;

                    // Encontrar pixel válido mais próximo
                    if let Some((r, g, b, a)) =
                        self.find_nearest_valid_pixel(image_data, mask, x, y, width, height)
                    {
                        result[pixel_idx] = r;
                        result[pixel_idx + 1] = g;
                        result[pixel_idx + 2] = b;
                        result[pixel_idx + 3] = a;
                    } else {
                        // Fallback: cor média da borda
                        let edge_color = self.compute_edge_average(image_data, mask, width, height);
                        result[pixel_idx] = edge_color.0;
                        result[pixel_idx + 1] = edge_color.1;
                        result[pixel_idx + 2] = edge_color.2;
                        result[pixel_idx + 3] = 255;
                    }
                }
            }
        }

        result
    }

    /// Encontra o pixel válido mais próximo
    fn find_nearest_valid_pixel(
        &self,
        image_data: &[u8],
        mask: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Option<(u8, u8, u8, u8)> {
        // Busca em espiral para encontrar pixel válido mais próximo
        for radius in 1..50 {
            for dy in -(radius as i32)..=(radius as i32) {
                for dx in -(radius as i32)..=(radius as i32) {
                    if dx.abs() != radius && dy.abs() != radius {
                        continue; // Só verificar borda do quadrado
                    }

                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let idx = (ny as u32 * width + nx as u32) as usize;
                        if mask[idx] == 0 {
                            let pixel_idx = idx * 4;
                            return Some((
                                image_data[pixel_idx],
                                image_data[pixel_idx + 1],
                                image_data[pixel_idx + 2],
                                image_data[pixel_idx + 3],
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    /// Calcula cor média da borda da imagem
    fn compute_edge_average(
        &self,
        image_data: &[u8],
        mask: &[u8],
        width: u32,
        height: u32,
    ) -> (u8, u8, u8) {
        let mut sum_r: u64 = 0;
        let mut sum_g: u64 = 0;
        let mut sum_b: u64 = 0;
        let mut count: u64 = 0;

        // Percorrer bordas da área válida (onde mask == 0)
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if mask[idx] == 0 {
                    // Verificar se é borda (adjacente a área de máscara)
                    let is_border = (x > 0 && mask[idx - 1] > 0)
                        || (x < width - 1 && mask[idx + 1] > 0)
                        || (y > 0 && mask[(idx as u32 - width) as usize] > 0)
                        || (y < height - 1 && mask[(idx as u32 + width) as usize] > 0);

                    if is_border {
                        let pixel_idx = idx * 4;
                        sum_r += image_data[pixel_idx] as u64;
                        sum_g += image_data[pixel_idx + 1] as u64;
                        sum_b += image_data[pixel_idx + 2] as u64;
                        count += 1;
                    }
                }
            }
        }

        if count > 0 {
            (
                (sum_r / count) as u8,
                (sum_g / count) as u8,
                (sum_b / count) as u8,
            )
        } else {
            (128, 128, 128) // Fallback para cinza médio
        }
    }
}

impl Default for OnnxInpainterImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IntelligentFillService for OnnxInpainterImpl {
    fn is_available(&self) -> bool {
        matches!(*self.status.lock().unwrap(), ModelStatus::Ready)
    }

    async fn fill(
        &self,
        image_data: &[u8],
        params: &IntelligentFillParams,
    ) -> DomainResult<IntelligentFillResult> {
        let start = std::time::Instant::now();

        let width = params.width;
        let height = params.height;

        // Gerar máscara de rotação
        let mask = self.generate_rotation_mask(width, height, params);

        // Verificar se há algo para preencher
        let needs_fill = mask.iter().any(|&v| v > 0);
        if !needs_fill {
            // Nenhuma área precisa de fill - retornar imagem original
            return Ok(IntelligentFillResult {
                image_data: image_data.to_vec(),
                width,
                height,
                process_time_ms: start.elapsed().as_secs_f32() * 1000.0,
            });
        }

        // Aplicar inpainting
        let filled_data = match *self.status.lock().unwrap() {
            ModelStatus::Ready => {
                // TODO: Usar inferência ONNX quando modelo estiver disponível
                // Por enquanto, usar fallback de extrapolação
                self.apply_edge_extrapolation(image_data, &mask, width, height)
            }
            _ => {
                // Modelo não disponível - usar fallback
                self.apply_edge_extrapolation(image_data, &mask, width, height)
            }
        };

        Ok(IntelligentFillResult {
            image_data: filled_data,
            width,
            height,
            process_time_ms: start.elapsed().as_secs_f32() * 1000.0,
        })
    }

    fn model_status(&self) -> ModelStatus {
        self.status.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::value_objects::CropSettings;

    #[test]
    fn test_generate_rotation_mask_zero_angle() {
        let inpainter = OnnxInpainterImpl::new();
        let params = IntelligentFillParams::preview(CropSettings::default(), 100, 100);

        let mask = inpainter.generate_rotation_mask(100, 100, &params);

        // Com ângulo 0, não deve haver área para preencher
        assert!(mask.iter().all(|&v| v == 0));
    }

    #[test]
    fn test_generate_rotation_mask_with_angle() {
        let inpainter = OnnxInpainterImpl::new();
        let crop = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 15.0, false, false);
        let params = IntelligentFillParams::preview(crop, 100, 100);

        let mask = inpainter.generate_rotation_mask(100, 100, &params);

        // Com ângulo de 15°, deve haver áreas para preencher nos cantos
        let fill_count = mask.iter().filter(|&&v| v > 0).count();
        assert!(fill_count > 0, "Deve haver áreas para preencher com rotação");
    }
}
