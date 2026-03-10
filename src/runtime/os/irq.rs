//! IRQ table — handler registration, enable/disable, dispatch.
//!
//! Provides a simulated interrupt controller for OS-level programming.
//! Handlers are stored by name and dispatched by IRQ number.
//! Supports priority levels and nested interrupt handling.

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Standard IRQ numbers
// ═══════════════════════════════════════════════════════════════════════

/// Timer interrupt (PIT/APIC).
pub const IRQ_TIMER: u8 = 0x20;
/// Keyboard interrupt.
pub const IRQ_KEYBOARD: u8 = 0x21;
/// Cascade (used internally by dual PIC).
pub const IRQ_CASCADE: u8 = 0x22;
/// Serial port COM2.
pub const IRQ_COM2: u8 = 0x23;
/// Serial port COM1.
pub const IRQ_COM1: u8 = 0x24;
/// Disk interrupt.
pub const IRQ_DISK: u8 = 0x2E;

// ═══════════════════════════════════════════════════════════════════════
// IRQ priority
// ═══════════════════════════════════════════════════════════════════════

/// Priority level for an IRQ handler. Higher numeric value = higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IrqPriority(pub u8);

impl IrqPriority {
    /// Lowest priority (background tasks).
    pub const LOW: Self = Self(0);
    /// Normal priority (default for most handlers).
    pub const NORMAL: Self = Self(128);
    /// High priority (time-critical handlers like timer).
    pub const HIGH: Self = Self(192);
    /// Critical priority (non-maskable, safety-critical).
    pub const CRITICAL: Self = Self(255);
}

impl Default for IrqPriority {
    fn default() -> Self {
        Self::NORMAL
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IRQ handler entry
// ═══════════════════════════════════════════════════════════════════════

/// An IRQ handler registration entry.
#[derive(Debug, Clone)]
pub struct IrqHandler {
    /// Name of the handler function.
    pub name: String,
    /// Priority level.
    pub priority: IrqPriority,
}

// ═══════════════════════════════════════════════════════════════════════
// IRQ errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors from IRQ operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum IrqError {
    /// No handler registered for this IRQ number.
    #[error("no handler for IRQ 0x{irq:02X}")]
    NoHandler { irq: u8 },

    /// IRQ number already has a handler.
    #[error("IRQ 0x{irq:02X} already has a handler")]
    AlreadyRegistered { irq: u8 },

    /// Interrupts are globally disabled.
    #[error("interrupts are disabled")]
    Disabled,

    /// Cannot nest: current IRQ has higher or equal priority.
    #[error(
        "IRQ 0x{irq:02X} (priority {irq_pri}) blocked by active IRQ with priority {active_pri}"
    )]
    PriorityBlocked {
        irq: u8,
        irq_pri: u8,
        active_pri: u8,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Critical section
// ═══════════════════════════════════════════════════════════════════════

/// Saved state for critical sections.
/// Call `enter_critical` to disable interrupts and `exit_critical` to restore.
#[derive(Debug, Clone, Copy)]
pub struct CriticalSectionGuard {
    /// Whether interrupts were enabled before entering the critical section.
    was_enabled: bool,
}

impl IrqTable {
    /// Enters a critical section: disables interrupts, returns guard to restore.
    pub fn enter_critical(&mut self) -> CriticalSectionGuard {
        let was_enabled = self.enabled;
        self.disable();
        CriticalSectionGuard { was_enabled }
    }

    /// Exits a critical section: restores interrupt state from the guard.
    pub fn exit_critical(&mut self, guard: CriticalSectionGuard) {
        if guard.was_enabled {
            self.enable();
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IRQ table
// ═══════════════════════════════════════════════════════════════════════

/// Simulated interrupt controller.
///
/// Maps IRQ numbers to handler function names with priority levels.
/// Supports nested interrupts: a higher-priority IRQ can preempt a
/// lower-priority handler.
#[derive(Debug)]
pub struct IrqTable {
    /// IRQ number → handler entry (name + priority).
    handlers: HashMap<u8, IrqHandler>,
    /// Global interrupt enable flag.
    enabled: bool,
    /// Stack of active IRQ priorities (for nesting).
    active_stack: Vec<IrqPriority>,
    /// Log of dispatched IRQs (for testing).
    dispatch_log: Vec<u8>,
}

impl IrqTable {
    /// Creates a new IRQ table with interrupts disabled.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            enabled: false,
            active_stack: Vec::new(),
            dispatch_log: Vec::new(),
        }
    }

    /// Registers a handler function for the given IRQ number with default priority.
    pub fn register(&mut self, irq: u8, handler_name: String) -> Result<(), IrqError> {
        self.register_with_priority(irq, handler_name, IrqPriority::default())
    }

    /// Registers a handler function with a specific priority.
    pub fn register_with_priority(
        &mut self,
        irq: u8,
        handler_name: String,
        priority: IrqPriority,
    ) -> Result<(), IrqError> {
        if self.handlers.contains_key(&irq) {
            return Err(IrqError::AlreadyRegistered { irq });
        }
        self.handlers.insert(
            irq,
            IrqHandler {
                name: handler_name,
                priority,
            },
        );
        Ok(())
    }

    /// Unregisters the handler for the given IRQ number.
    pub fn unregister(&mut self, irq: u8) -> Result<(), IrqError> {
        if self.handlers.remove(&irq).is_none() {
            return Err(IrqError::NoHandler { irq });
        }
        Ok(())
    }

    /// Enables interrupts globally.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disables interrupts globally.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Returns whether interrupts are globally enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the current nesting depth (0 = no active IRQ).
    pub fn nesting_depth(&self) -> usize {
        self.active_stack.len()
    }

    /// Returns the priority of the currently active IRQ, if any.
    pub fn active_priority(&self) -> Option<IrqPriority> {
        self.active_stack.last().copied()
    }

    /// Dispatches an IRQ, returning the handler function name if available.
    ///
    /// Returns `Err(Disabled)` if interrupts are globally disabled,
    /// or `Err(NoHandler)` if no handler is registered for this IRQ.
    pub fn dispatch(&mut self, irq: u8) -> Result<String, IrqError> {
        if !self.enabled {
            return Err(IrqError::Disabled);
        }
        match self.handlers.get(&irq) {
            Some(handler) => {
                // Check priority for nesting
                if let Some(&active_pri) = self.active_stack.last() {
                    if handler.priority <= active_pri {
                        return Err(IrqError::PriorityBlocked {
                            irq,
                            irq_pri: handler.priority.0,
                            active_pri: active_pri.0,
                        });
                    }
                }
                self.active_stack.push(handler.priority);
                self.dispatch_log.push(irq);
                Ok(handler.name.clone())
            }
            None => Err(IrqError::NoHandler { irq }),
        }
    }

    /// Marks the current IRQ handler as complete, popping the nesting stack.
    pub fn end_of_interrupt(&mut self) {
        self.active_stack.pop();
    }

    /// Returns the handler name for the given IRQ, if registered.
    pub fn handler_for(&self, irq: u8) -> Option<&str> {
        self.handlers.get(&irq).map(|h| h.name.as_str())
    }

    /// Returns the priority for the given IRQ, if registered.
    pub fn priority_for(&self, irq: u8) -> Option<IrqPriority> {
        self.handlers.get(&irq).map(|h| h.priority)
    }

    /// Returns the number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// Returns the dispatch log (for testing).
    pub fn dispatch_log(&self) -> &[u8] {
        &self.dispatch_log
    }

    /// Returns all registered IRQs sorted by priority (highest first).
    pub fn handlers_by_priority(&self) -> Vec<(u8, &IrqHandler)> {
        let mut entries: Vec<_> = self.handlers.iter().map(|(&irq, h)| (irq, h)).collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.1.priority));
        entries
    }
}

impl Default for IrqTable {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_table_has_no_handlers() {
        let table = IrqTable::new();
        assert_eq!(table.handler_count(), 0);
        assert!(!table.is_enabled());
    }

    #[test]
    fn register_and_lookup() {
        let mut table = IrqTable::new();
        table.register(IRQ_TIMER, "timer_handler".into()).unwrap();
        assert_eq!(table.handler_for(IRQ_TIMER), Some("timer_handler"));
        assert_eq!(table.handler_count(), 1);
    }

    #[test]
    fn register_multiple() {
        let mut table = IrqTable::new();
        table.register(IRQ_TIMER, "timer_handler".into()).unwrap();
        table.register(IRQ_KEYBOARD, "kb_handler".into()).unwrap();
        assert_eq!(table.handler_count(), 2);
    }

    #[test]
    fn register_duplicate_fails() {
        let mut table = IrqTable::new();
        table.register(IRQ_TIMER, "timer_handler".into()).unwrap();
        assert!(matches!(
            table.register(IRQ_TIMER, "other".into()),
            Err(IrqError::AlreadyRegistered { irq: IRQ_TIMER })
        ));
    }

    #[test]
    fn unregister() {
        let mut table = IrqTable::new();
        table.register(IRQ_TIMER, "timer_handler".into()).unwrap();
        table.unregister(IRQ_TIMER).unwrap();
        assert_eq!(table.handler_count(), 0);
        assert_eq!(table.handler_for(IRQ_TIMER), None);
    }

    #[test]
    fn unregister_nonexistent_fails() {
        let mut table = IrqTable::new();
        assert!(matches!(
            table.unregister(IRQ_TIMER),
            Err(IrqError::NoHandler { .. })
        ));
    }

    #[test]
    fn enable_disable() {
        let mut table = IrqTable::new();
        assert!(!table.is_enabled());
        table.enable();
        assert!(table.is_enabled());
        table.disable();
        assert!(!table.is_enabled());
    }

    #[test]
    fn dispatch_when_disabled() {
        let mut table = IrqTable::new();
        table.register(IRQ_TIMER, "timer_handler".into()).unwrap();
        assert!(matches!(table.dispatch(IRQ_TIMER), Err(IrqError::Disabled)));
    }

    #[test]
    fn dispatch_no_handler() {
        let mut table = IrqTable::new();
        table.enable();
        assert!(matches!(
            table.dispatch(IRQ_TIMER),
            Err(IrqError::NoHandler { .. })
        ));
    }

    #[test]
    fn dispatch_success() {
        let mut table = IrqTable::new();
        table.register(IRQ_KEYBOARD, "kb_handler".into()).unwrap();
        table.enable();
        let handler = table.dispatch(IRQ_KEYBOARD).unwrap();
        assert_eq!(handler, "kb_handler");
        table.end_of_interrupt();
    }

    #[test]
    fn dispatch_log_tracks_irqs() {
        let mut table = IrqTable::new();
        table.register(IRQ_TIMER, "timer_handler".into()).unwrap();
        table.register(IRQ_KEYBOARD, "kb_handler".into()).unwrap();
        table.enable();
        table.dispatch(IRQ_TIMER).unwrap();
        table.end_of_interrupt();
        table.dispatch(IRQ_KEYBOARD).unwrap();
        table.end_of_interrupt();
        table.dispatch(IRQ_TIMER).unwrap();
        table.end_of_interrupt();
        assert_eq!(table.dispatch_log(), &[IRQ_TIMER, IRQ_KEYBOARD, IRQ_TIMER]);
    }

    #[test]
    fn standard_irq_numbers() {
        assert_eq!(IRQ_TIMER, 0x20);
        assert_eq!(IRQ_KEYBOARD, 0x21);
        assert_eq!(IRQ_COM1, 0x24);
        assert_eq!(IRQ_DISK, 0x2E);
    }

    // ── Priority tests ──────────────────────────────────────────────

    #[test]
    fn register_with_priority() {
        let mut table = IrqTable::new();
        table
            .register_with_priority(IRQ_TIMER, "timer".into(), IrqPriority::HIGH)
            .unwrap();
        assert_eq!(table.priority_for(IRQ_TIMER), Some(IrqPriority::HIGH));
    }

    #[test]
    fn default_priority_is_normal() {
        let mut table = IrqTable::new();
        table.register(IRQ_KEYBOARD, "kb".into()).unwrap();
        assert_eq!(table.priority_for(IRQ_KEYBOARD), Some(IrqPriority::NORMAL));
    }

    #[test]
    fn priority_ordering() {
        assert!(IrqPriority::LOW < IrqPriority::NORMAL);
        assert!(IrqPriority::NORMAL < IrqPriority::HIGH);
        assert!(IrqPriority::HIGH < IrqPriority::CRITICAL);
    }

    #[test]
    fn handlers_sorted_by_priority() {
        let mut table = IrqTable::new();
        table
            .register_with_priority(IRQ_KEYBOARD, "kb".into(), IrqPriority::LOW)
            .unwrap();
        table
            .register_with_priority(IRQ_TIMER, "timer".into(), IrqPriority::CRITICAL)
            .unwrap();
        table
            .register_with_priority(IRQ_COM1, "com1".into(), IrqPriority::NORMAL)
            .unwrap();

        let sorted = table.handlers_by_priority();
        assert_eq!(sorted[0].0, IRQ_TIMER); // CRITICAL first
        assert_eq!(sorted[1].0, IRQ_COM1); // NORMAL second
        assert_eq!(sorted[2].0, IRQ_KEYBOARD); // LOW last
    }

    // ── Nested interrupt tests ──────────────────────────────────────

    #[test]
    fn nesting_depth_starts_at_zero() {
        let table = IrqTable::new();
        assert_eq!(table.nesting_depth(), 0);
        assert_eq!(table.active_priority(), None);
    }

    #[test]
    fn nested_higher_priority_allowed() {
        let mut table = IrqTable::new();
        table
            .register_with_priority(IRQ_KEYBOARD, "kb".into(), IrqPriority::NORMAL)
            .unwrap();
        table
            .register_with_priority(IRQ_TIMER, "timer".into(), IrqPriority::HIGH)
            .unwrap();
        table.enable();

        // Start handling keyboard (NORMAL priority)
        table.dispatch(IRQ_KEYBOARD).unwrap();
        assert_eq!(table.nesting_depth(), 1);

        // Timer (HIGH) preempts keyboard (NORMAL)
        table.dispatch(IRQ_TIMER).unwrap();
        assert_eq!(table.nesting_depth(), 2);

        // End timer handler
        table.end_of_interrupt();
        assert_eq!(table.nesting_depth(), 1);

        // End keyboard handler
        table.end_of_interrupt();
        assert_eq!(table.nesting_depth(), 0);
    }

    #[test]
    fn nested_lower_priority_blocked() {
        let mut table = IrqTable::new();
        table
            .register_with_priority(IRQ_KEYBOARD, "kb".into(), IrqPriority::LOW)
            .unwrap();
        table
            .register_with_priority(IRQ_TIMER, "timer".into(), IrqPriority::HIGH)
            .unwrap();
        table.enable();

        // Start handling timer (HIGH priority)
        table.dispatch(IRQ_TIMER).unwrap();

        // Keyboard (LOW) cannot preempt timer (HIGH)
        let result = table.dispatch(IRQ_KEYBOARD);
        assert!(matches!(result, Err(IrqError::PriorityBlocked { .. })));

        table.end_of_interrupt();
    }

    #[test]
    fn nested_equal_priority_blocked() {
        let mut table = IrqTable::new();
        table
            .register_with_priority(IRQ_KEYBOARD, "kb".into(), IrqPriority::NORMAL)
            .unwrap();
        table
            .register_with_priority(IRQ_COM1, "com1".into(), IrqPriority::NORMAL)
            .unwrap();
        table.enable();

        table.dispatch(IRQ_KEYBOARD).unwrap();
        let result = table.dispatch(IRQ_COM1);
        assert!(matches!(result, Err(IrqError::PriorityBlocked { .. })));
        table.end_of_interrupt();
    }

    // ── Critical section tests ──────────────────────────────────────

    #[test]
    fn critical_section_disables_interrupts() {
        let mut table = IrqTable::new();
        table.enable();
        assert!(table.is_enabled());

        let guard = table.enter_critical();
        assert!(!table.is_enabled());

        table.exit_critical(guard);
        assert!(table.is_enabled());
    }

    #[test]
    fn critical_section_preserves_disabled_state() {
        let mut table = IrqTable::new();
        assert!(!table.is_enabled());

        let guard = table.enter_critical();
        assert!(!table.is_enabled());

        table.exit_critical(guard);
        // Should stay disabled since they were disabled before
        assert!(!table.is_enabled());
    }
}
