use std::ops::Range;

use crate::ModelDistribution;

pub async fn get_layers_distribution(endpoint: &str, model: &str) -> Result<ModelDistribution, String> {
    let http_endpoint = endpoint.replace("ws://", "http://").replace("wss://", "https://").replace("/ws", "/api");
    let url = format!("{http_endpoint}/{model}/distribution");
    log::info!("[Registry] get_layers_distribution url: {}", url);
    let resp = reqwest::get(url).await.map_err(|e| e.to_string())?;
    resp.json::<ModelDistribution>().await.map_err(|e| e.to_string())
}

#[derive(Debug, PartialEq, Eq)]
pub enum LayerSelectionRes {
    EnoughLayers { ranges: Range<u32> },
    NotEnoughLayers { min_layers: u32 },
}

pub fn select_layers(distribution: &[usize], layers: u32) -> LayerSelectionRes {
    let start_to_layers = distribution.iter().position(|c| *c == 0);
    let end_to_layers = distribution.iter().rev().position(|c| *c == 0);

    match (start_to_layers, end_to_layers) {
        (Some(start), Some(end)) => {
            let min_layers = (distribution.len() - end - 1) as u32 - start as u32 + 1;
            if min_layers > layers {
                return LayerSelectionRes::NotEnoughLayers {
                    min_layers: (distribution.len() - end - 1) as u32 - start as u32 + 1,
                };
            }
        }
        _ => {}
    }

    // we select best layers by sum of continunes n layers, with n is the number of layers
    // then select the first smallest range
    let sum_layers = sum_in_window(distribution, layers as usize);
    let min_sum = sum_layers.iter().min().unwrap();
    let index = sum_layers.iter().position(|x| *x == *min_sum).unwrap();
    LayerSelectionRes::EnoughLayers {
        ranges: index as u32..index as u32 + layers,
    }
}

fn sum_in_window(data: &[usize], window_size: usize) -> Vec<usize> {
    let mut result = Vec::new();
    for i in 0..(data.len() - window_size + 1) {
        let mut sum = 0;
        for j in 0..window_size {
            if i + j < data.len() {
                sum += data[i + j];
            }
        }
        result.push(sum);
    }
    result
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_sum_in_window() {
        let data = vec![1, 1, 1, 1];
        let window_size = 2;
        let result = sum_in_window(&data, window_size);
        assert_eq!(result, vec![2, 2, 2]);
    }

    #[test]
    fn test_select_layers() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let layers = 3;
        let result = select_layers(&data, layers);
        assert_eq!(result, LayerSelectionRes::EnoughLayers { ranges: 0..3 });
    }

    #[test]
    fn test_select_layers_not_enough() {
        let data = vec![1, 1, 0, 0, 0, 0, 1, 1, 1, 1];
        let layers = 2;
        let result = select_layers(&data, layers);
        assert_eq!(result, LayerSelectionRes::NotEnoughLayers { min_layers: 4 });
    }

    #[test]
    fn test_select_overlapped_layers() {
        let data = vec![1, 1, 0, 0, 1, 1, 1, 1, 1, 1];
        let layers = 3;
        let result = select_layers(&data, layers);
        assert_eq!(result, LayerSelectionRes::EnoughLayers { ranges: 1..4 });
    }

    #[test]
    fn test_select_layers_with_min() {
        let data = vec![1, 2, 3, 1, 1, 1, 1, 1, 2, 1];
        let layers = 3;
        let result = select_layers(&data, layers);
        assert_eq!(result, LayerSelectionRes::EnoughLayers { ranges: 3..6 });
    }
}
