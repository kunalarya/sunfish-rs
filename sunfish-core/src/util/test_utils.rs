#[cfg(test)]
pub fn assert_similar_f32(a: &Vec<f32>, b: &Vec<f32>, threshold: f32) {
    let a_rounded: Vec<f32> = a
        .iter()
        .map(|f| (f * (threshold)).round() / threshold)
        .collect();
    let b_rounded: Vec<f32> = b
        .iter()
        .map(|f| (f * (threshold)).round() / threshold)
        .collect();
    assert_eq!(a_rounded, b_rounded);
}

#[cfg(test)]
pub fn assert_similar_f64(a: &Vec<f64>, b: &Vec<f64>, threshold: f64) {
    let a_rounded: Vec<f64> = a
        .iter()
        .map(|f| (f * (threshold)).round() / threshold)
        .collect();
    let b_rounded: Vec<f64> = b
        .iter()
        .map(|f| (f * (threshold)).round() / threshold)
        .collect();
    assert_eq!(a_rounded, b_rounded);
}
