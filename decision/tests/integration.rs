//! Integration tests for decision crate

use agent_decision::task_preparation::TaskPreparationPipeline;

#[test]
fn test_task_preparation_pipeline_creation() {
    // Integration test: TaskPreparationPipeline requires git operations
    // so it belongs in integration tests rather than unit tests
    let _pipeline = TaskPreparationPipeline::new();
    // Just verify it can be created
    assert!(true);
}
