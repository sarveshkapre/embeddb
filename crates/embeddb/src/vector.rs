use crate::DistanceMetric;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub row_id: u64,
    pub distance: f32,
}

pub fn distance(query: &[f32], vector: &[f32], metric: DistanceMetric) -> f32 {
    if query.len() != vector.len() || query.is_empty() {
        return f32::INFINITY;
    }

    match metric {
        DistanceMetric::L2 => l2_distance(query, vector),
        DistanceMetric::Cosine => cosine_distance(query, vector),
    }
}

fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        let diff = x - y;
        sum += diff * diff;
    }
    sum
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    1.0 - (dot / denom)
}
