use super::*;

fn ms(values: &[u64]) -> Vec<Duration> {
    values.iter().map(|v| Duration::from_millis(*v)).collect()
}

#[test]
fn percentiles_use_the_nearest_rank() {
    let samples = ms(&[500, 100, 400, 200, 300]);
    assert_eq!(percentile(&samples, 0.5), Some(Duration::from_millis(300)));
    assert_eq!(percentile(&samples, 0.95), Some(Duration::from_millis(500)));
    assert_eq!(percentile(&samples, 0.0), Some(Duration::from_millis(100)));
    assert_eq!(percentile(&[], 0.5), None);
}
