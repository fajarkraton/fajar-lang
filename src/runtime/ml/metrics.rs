//! ML metrics — accuracy, precision, recall, F1 score.
//!
//! Operates on prediction and label arrays for classification tasks.

/// Computes classification accuracy: fraction of correct predictions.
pub fn accuracy(predictions: &[i64], labels: &[i64]) -> f64 {
    if predictions.is_empty() {
        return 0.0;
    }
    let correct = predictions
        .iter()
        .zip(labels.iter())
        .filter(|(p, l)| p == l)
        .count();
    correct as f64 / predictions.len() as f64
}

/// Computes precision for a specific class: TP / (TP + FP).
pub fn precision(predictions: &[i64], labels: &[i64], class: i64) -> f64 {
    let tp = predictions
        .iter()
        .zip(labels.iter())
        .filter(|(p, l)| **p == class && **l == class)
        .count();
    let fp = predictions
        .iter()
        .zip(labels.iter())
        .filter(|(p, l)| **p == class && **l != class)
        .count();
    if tp + fp == 0 {
        return 0.0;
    }
    tp as f64 / (tp + fp) as f64
}

/// Computes recall for a specific class: TP / (TP + FN).
pub fn recall(predictions: &[i64], labels: &[i64], class: i64) -> f64 {
    let tp = predictions
        .iter()
        .zip(labels.iter())
        .filter(|(p, l)| **p == class && **l == class)
        .count();
    let fn_count = predictions
        .iter()
        .zip(labels.iter())
        .filter(|(p, l)| **p != class && **l == class)
        .count();
    if tp + fn_count == 0 {
        return 0.0;
    }
    tp as f64 / (tp + fn_count) as f64
}

/// Computes F1 score for a specific class: 2 * (precision * recall) / (precision + recall).
pub fn f1_score(predictions: &[i64], labels: &[i64], class: i64) -> f64 {
    let p = precision(predictions, labels, class);
    let r = recall(predictions, labels, class);
    if p + r == 0.0 {
        return 0.0;
    }
    2.0 * p * r / (p + r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accuracy_perfect() {
        let preds = vec![0, 1, 2, 0, 1];
        let labels = vec![0, 1, 2, 0, 1];
        assert!((accuracy(&preds, &labels) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn accuracy_half() {
        let preds = vec![0, 1, 0, 1];
        let labels = vec![0, 0, 0, 0];
        assert!((accuracy(&preds, &labels) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn accuracy_empty() {
        assert!((accuracy(&[], &[]) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn precision_basic() {
        // preds: [1, 1, 0, 0], labels: [1, 0, 0, 1]
        // For class 1: TP=1 (pos 0), FP=1 (pos 1), precision=0.5
        let preds = vec![1, 1, 0, 0];
        let labels = vec![1, 0, 0, 1];
        assert!((precision(&preds, &labels, 1) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn recall_basic() {
        // preds: [1, 1, 0, 0], labels: [1, 0, 0, 1]
        // For class 1: TP=1, FN=1 (pos 3), recall=0.5
        let preds = vec![1, 1, 0, 0];
        let labels = vec![1, 0, 0, 1];
        assert!((recall(&preds, &labels, 1) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn f1_score_basic() {
        let preds = vec![1, 1, 0, 0];
        let labels = vec![1, 0, 0, 1];
        // precision=0.5, recall=0.5, f1=0.5
        assert!((f1_score(&preds, &labels, 1) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn precision_no_predictions() {
        let preds = vec![0, 0, 0];
        let labels = vec![1, 1, 1];
        assert!((precision(&preds, &labels, 1) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn recall_no_true_positives() {
        let preds = vec![0, 0, 0];
        let labels = vec![0, 0, 0];
        // class 1: no labels, so recall = 0
        assert!((recall(&preds, &labels, 1) - 0.0).abs() < 1e-10);
    }
}
