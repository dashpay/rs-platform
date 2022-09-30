use super::fee::operations::Operation;

#[derive(Debug, Clone, Default)]

pub struct StateTransitionExecutionContext {
    actual_operations: Vec<Operation>,
    dry_run_operations: Vec<Operation>,
    is_dry_run: bool,
}

impl StateTransitionExecutionContext {
    /// Add [`Operation`] into execution context
    pub fn add_operation(&mut self, operation: Operation) {
        if self.is_dry_run {
            self.dry_run_operations.push(operation);
        } else {
            self.actual_operations.push(operation);
        }
    }

    /// Replace all existing operations with a new collection of operations
    pub fn set_operations(&mut self, operations: Vec<Operation>) {
        self.actual_operations = operations
    }

    /// Returns all (actual & dry run) operations
    pub fn get_operations(&self) -> impl Iterator<Item = &Operation> {
        self.actual_operations
            .iter()
            .chain(self.dry_run_operations.iter())
    }

    /// Enable dry run
    pub fn enable_dry_run(&mut self) {
        self.is_dry_run = true;
    }

    /// Disable dry run
    pub fn disable_dry_run(&mut self) {
        self.is_dry_run = false;
    }

    pub fn clear_dry_run_operations(&mut self) {
        self.dry_run_operations.clear()
    }

    pub fn is_dry_run(&self) -> bool {
        self.is_dry_run
    }
}
