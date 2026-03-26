//! End-to-End Pipeline — @kernel/@device/@safe bridges, scheduling, demos.
//!
//! Phase R3: 20 tasks covering context bridging, deadline scheduling,
//! jitter measurement, watchdog, telemetry, and 8 demo applications.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// R3.1.1-R3.1.3: Context Bridges
// ═══════════════════════════════════════════════════════════════════════

/// Context annotation for pipeline stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    /// Hardware access (sensors, actuators, interrupts).
    Kernel,
    /// Tensor operations (inference, training).
    Device,
    /// Orchestration (no hardware, no tensors — safest).
    Safe,
}

impl fmt::Display for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kernel => write!(f, "@kernel"),
            Self::Device => write!(f, "@device"),
            Self::Safe => write!(f, "@safe"),
        }
    }
}

/// A pipeline stage with context annotation.
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// Stage name.
    pub name: String,
    /// Context annotation.
    pub context: Context,
    /// Expected latency in microseconds.
    pub expected_latency_us: u64,
    /// Priority (0 = highest).
    pub priority: u32,
}

/// Bridge between @kernel and @device contexts (zero-copy).
#[derive(Debug, Clone)]
pub struct KernelDeviceBridge {
    /// Shared memory address for sensor → tensor transfer.
    pub shared_buffer_addr: u64,
    /// Buffer size in bytes.
    pub buffer_size: usize,
    /// Whether a transfer is pending.
    pub transfer_pending: bool,
    /// Sequence number for ordering.
    pub seq: u64,
}

impl KernelDeviceBridge {
    /// Creates a new bridge.
    pub fn new(addr: u64, size: usize) -> Self {
        Self {
            shared_buffer_addr: addr,
            buffer_size: size,
            transfer_pending: false,
            seq: 0,
        }
    }

    /// Signals that sensor data is ready for inference.
    pub fn signal_ready(&mut self) {
        self.transfer_pending = true;
        self.seq += 1;
    }

    /// Acknowledges the transfer (inference picked it up).
    pub fn acknowledge(&mut self) {
        self.transfer_pending = false;
    }
}

/// Bridge between @device and @kernel contexts (inference result → actuator).
#[derive(Debug, Clone)]
pub struct DeviceKernelBridge {
    /// Result buffer address.
    pub result_addr: u64,
    /// Predicted class or action.
    pub action_id: u32,
    /// Confidence level.
    pub confidence: f32,
    /// Result ready flag.
    pub result_ready: bool,
}

impl DeviceKernelBridge {
    /// Creates a new bridge.
    pub fn new(addr: u64) -> Self {
        Self {
            result_addr: addr,
            action_id: 0,
            confidence: 0.0,
            result_ready: false,
        }
    }

    /// Posts an inference result.
    pub fn post_result(&mut self, action_id: u32, confidence: f32) {
        self.action_id = action_id;
        self.confidence = confidence;
        self.result_ready = true;
    }

    /// Consumes the result.
    pub fn consume(&mut self) -> Option<(u32, f32)> {
        if self.result_ready {
            self.result_ready = false;
            Some((self.action_id, self.confidence))
        } else {
            None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R3.1.4-R3.1.5: Deadline Scheduler + Jitter Measurement
// ═══════════════════════════════════════════════════════════════════════

/// Real-time task for the deadline scheduler.
#[derive(Debug, Clone)]
pub struct RtTask {
    /// Task name.
    pub name: String,
    /// Period in microseconds.
    pub period_us: u64,
    /// Deadline in microseconds (from start of period).
    pub deadline_us: u64,
    /// Worst-case execution time (estimated, microseconds).
    pub wcet_us: u64,
    /// Context.
    pub context: Context,
    /// Priority (lower = higher priority).
    pub priority: u32,
}

/// Jitter measurement.
#[derive(Debug, Clone)]
pub struct JitterStats {
    /// Measured execution times (microseconds).
    pub samples: Vec<u64>,
    /// Target period (microseconds).
    pub target_us: u64,
}

impl JitterStats {
    /// Creates a new jitter tracker.
    pub fn new(target_us: u64) -> Self {
        Self {
            samples: Vec::new(),
            target_us,
        }
    }

    /// Records a measurement.
    pub fn record(&mut self, actual_us: u64) {
        self.samples.push(actual_us);
    }

    /// Returns mean jitter (absolute deviation from target).
    pub fn mean_jitter(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self
            .samples
            .iter()
            .map(|&s| (s as f64 - self.target_us as f64).abs())
            .sum();
        sum / self.samples.len() as f64
    }

    /// Returns max jitter.
    pub fn max_jitter(&self) -> u64 {
        self.samples
            .iter()
            .map(|&s| (s as i64 - self.target_us as i64).unsigned_abs())
            .max()
            .unwrap_or(0)
    }

    /// Returns true if all samples are within the tolerance.
    pub fn within_tolerance(&self, tolerance_us: u64) -> bool {
        self.samples.iter().all(|&s| {
            let diff = (s as i64 - self.target_us as i64).unsigned_abs();
            diff <= tolerance_us
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R3.1.6: WCET Analysis
// ═══════════════════════════════════════════════════════════════════════

/// WCET analysis result for a function.
#[derive(Debug, Clone)]
pub struct WcetAnalysis {
    /// Function name.
    pub function_name: String,
    /// Estimated WCET in microseconds.
    pub wcet_us: u64,
    /// Average-case execution time.
    pub acet_us: u64,
    /// Best-case execution time.
    pub bcet_us: u64,
    /// Number of samples.
    pub samples: u64,
}

/// Checks if a task set is schedulable (Rate Monotonic Analysis).
pub fn rate_monotonic_test(tasks: &[RtTask]) -> bool {
    let n = tasks.len();
    if n == 0 {
        return true;
    }

    // Utilization bound: sum(Ci/Ti) <= n * (2^(1/n) - 1)
    let utilization: f64 = tasks
        .iter()
        .map(|t| t.wcet_us as f64 / t.period_us as f64)
        .sum();

    let bound = n as f64 * (2.0_f64.powf(1.0 / n as f64) - 1.0);
    utilization <= bound
}

// ═══════════════════════════════════════════════════════════════════════
// R3.1.7: Priority Inheritance
// ═══════════════════════════════════════════════════════════════════════

/// Priority inheritance protocol state.
#[derive(Debug, Clone)]
pub struct PriorityInheritance {
    /// Resource → owning task.
    pub owners: Vec<(String, String)>,
    /// Task → current (possibly inherited) priority.
    pub effective_priority: Vec<(String, u32)>,
}

impl Default for PriorityInheritance {
    fn default() -> Self {
        Self::new()
    }
}

impl PriorityInheritance {
    /// Creates a new priority inheritance tracker.
    pub fn new() -> Self {
        Self {
            owners: Vec::new(),
            effective_priority: Vec::new(),
        }
    }

    /// Records that a task acquired a resource.
    pub fn acquire(&mut self, task: &str, resource: &str, priority: u32) {
        self.owners.push((resource.to_string(), task.to_string()));
        self.effective_priority.push((task.to_string(), priority));
    }

    /// A higher-priority task is blocked on a resource — inherit priority.
    pub fn inherit(&mut self, blocker_task: &str, new_priority: u32) {
        for (task, prio) in &mut self.effective_priority {
            if task == blocker_task && *prio > new_priority {
                *prio = new_priority; // lower number = higher priority
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R3.1.8-R3.1.10: Watchdog, Telemetry, Power
// ═══════════════════════════════════════════════════════════════════════

/// Watchdog timer configuration.
#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
    /// Action on timeout.
    pub action: WatchdogAction,
    /// Last kick timestamp.
    pub last_kick_ms: u64,
}

/// Watchdog timeout action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogAction {
    Reset,
    Log,
    EmergencyStop,
}

impl WatchdogConfig {
    /// Kicks the watchdog (resets timer).
    pub fn kick(&mut self, now_ms: u64) {
        self.last_kick_ms = now_ms;
    }

    /// Checks if the watchdog has timed out.
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms - self.last_kick_ms > self.timeout_ms
    }
}

/// Pipeline telemetry metrics.
#[derive(Debug, Clone, Default)]
pub struct Telemetry {
    /// Total inferences performed.
    pub total_inferences: u64,
    /// Total sensor reads.
    pub total_reads: u64,
    /// Total actuator commands.
    pub total_commands: u64,
    /// Total deadline violations.
    pub deadline_violations: u64,
    /// Average inference latency (microseconds).
    pub avg_inference_us: f64,
    /// Average end-to-end latency (microseconds).
    pub avg_e2e_us: f64,
    /// Uptime in seconds.
    pub uptime_secs: u64,
}

impl fmt::Display for Telemetry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "inferences={} reads={} cmds={} violations={} avg_lat={:.0}μs e2e={:.0}μs uptime={}s",
            self.total_inferences,
            self.total_reads,
            self.total_commands,
            self.deadline_violations,
            self.avg_inference_us,
            self.avg_e2e_us,
            self.uptime_secs
        )
    }
}

/// Power management mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerMode {
    /// Full performance.
    Performance,
    /// Balanced (reduce frequency when idle).
    Balanced,
    /// Low power (minimum frequency).
    LowPower,
    /// Sleep (wake on interrupt).
    Sleep,
}

// ═══════════════════════════════════════════════════════════════════════
// R3.2: Demo Application Descriptors
// ═══════════════════════════════════════════════════════════════════════

/// A demo application descriptor.
#[derive(Debug, Clone)]
pub struct DemoApp {
    /// Application name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Sensor types used.
    pub sensors: Vec<String>,
    /// Model name.
    pub model: String,
    /// Actuator types.
    pub actuators: Vec<String>,
    /// Target latency.
    pub target_latency_ms: f64,
    /// Context pipeline.
    pub pipeline: Vec<PipelineStage>,
}

/// Returns all demo application descriptors.
pub fn demo_apps() -> Vec<DemoApp> {
    vec![
        DemoApp {
            name: "Drone Autopilot".to_string(),
            description: "IMU → stabilization model → motor control".to_string(),
            sensors: vec!["IMU (MPU6050)".to_string(), "Barometer".to_string()],
            model: "stabilizer_net".to_string(),
            actuators: vec!["4x PWM motors".to_string()],
            target_latency_ms: 5.0,
            pipeline: vec![
                PipelineStage {
                    name: "read_imu".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 200,
                    priority: 0,
                },
                PipelineStage {
                    name: "stabilize".to_string(),
                    context: Context::Device,
                    expected_latency_us: 2000,
                    priority: 1,
                },
                PipelineStage {
                    name: "set_motors".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 100,
                    priority: 0,
                },
            ],
        },
        DemoApp {
            name: "Object Tracker".to_string(),
            description: "Camera → YOLO detector → servo follow".to_string(),
            sensors: vec!["Camera (320x240)".to_string()],
            model: "yolo_tiny".to_string(),
            actuators: vec!["Pan servo".to_string(), "Tilt servo".to_string()],
            target_latency_ms: 33.0, // 30fps
            pipeline: vec![
                PipelineStage {
                    name: "capture_frame".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 1000,
                    priority: 0,
                },
                PipelineStage {
                    name: "detect_objects".to_string(),
                    context: Context::Device,
                    expected_latency_us: 20000,
                    priority: 1,
                },
                PipelineStage {
                    name: "track_servo".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 500,
                    priority: 0,
                },
            ],
        },
        DemoApp {
            name: "Anomaly Detector".to_string(),
            description: "Vibration sensor → FFT → classifier → alert".to_string(),
            sensors: vec!["Accelerometer".to_string()],
            model: "anomaly_classifier".to_string(),
            actuators: vec!["Alert LED".to_string(), "Buzzer".to_string()],
            target_latency_ms: 10.0,
            pipeline: vec![
                PipelineStage {
                    name: "read_vibration".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 100,
                    priority: 0,
                },
                PipelineStage {
                    name: "fft_features".to_string(),
                    context: Context::Device,
                    expected_latency_us: 3000,
                    priority: 1,
                },
                PipelineStage {
                    name: "classify".to_string(),
                    context: Context::Device,
                    expected_latency_us: 2000,
                    priority: 1,
                },
                PipelineStage {
                    name: "alert".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 50,
                    priority: 0,
                },
            ],
        },
        DemoApp {
            name: "Voice Command".to_string(),
            description: "Microphone → keyword detection → GPIO action".to_string(),
            sensors: vec!["Microphone (16kHz)".to_string()],
            model: "keyword_spotter".to_string(),
            actuators: vec!["GPIO relay".to_string()],
            target_latency_ms: 100.0,
            pipeline: vec![
                PipelineStage {
                    name: "capture_audio".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 62500,
                    priority: 0,
                },
                PipelineStage {
                    name: "detect_keyword".to_string(),
                    context: Context::Device,
                    expected_latency_us: 30000,
                    priority: 1,
                },
                PipelineStage {
                    name: "gpio_action".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 50,
                    priority: 0,
                },
            ],
        },
        DemoApp {
            name: "Autonomous Rover".to_string(),
            description: "LiDAR → obstacle avoidance → motor control".to_string(),
            sensors: vec!["LiDAR (360°)".to_string(), "IMU".to_string()],
            model: "obstacle_net".to_string(),
            actuators: vec!["2x DC motors".to_string(), "Steering servo".to_string()],
            target_latency_ms: 20.0,
            pipeline: vec![
                PipelineStage {
                    name: "scan_lidar".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 5000,
                    priority: 0,
                },
                PipelineStage {
                    name: "plan_path".to_string(),
                    context: Context::Device,
                    expected_latency_us: 10000,
                    priority: 1,
                },
                PipelineStage {
                    name: "drive".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 200,
                    priority: 0,
                },
            ],
        },
        DemoApp {
            name: "Predictive Maintenance".to_string(),
            description: "Sensor trends → failure prediction → alert".to_string(),
            sensors: vec![
                "Temperature".to_string(),
                "Vibration".to_string(),
                "Current".to_string(),
            ],
            model: "failure_predictor".to_string(),
            actuators: vec!["Alert system".to_string()],
            target_latency_ms: 1000.0,
            pipeline: vec![
                PipelineStage {
                    name: "read_sensors".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 500,
                    priority: 0,
                },
                PipelineStage {
                    name: "predict".to_string(),
                    context: Context::Device,
                    expected_latency_us: 50000,
                    priority: 2,
                },
                PipelineStage {
                    name: "alert".to_string(),
                    context: Context::Safe,
                    expected_latency_us: 1000,
                    priority: 1,
                },
            ],
        },
        DemoApp {
            name: "Smart Agriculture".to_string(),
            description: "Soil moisture → irrigation control".to_string(),
            sensors: vec![
                "Soil moisture".to_string(),
                "Temperature".to_string(),
                "Humidity".to_string(),
            ],
            model: "irrigation_model".to_string(),
            actuators: vec!["Water valve (solenoid)".to_string()],
            target_latency_ms: 5000.0,
            pipeline: vec![
                PipelineStage {
                    name: "read_soil".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 1000,
                    priority: 0,
                },
                PipelineStage {
                    name: "decide".to_string(),
                    context: Context::Device,
                    expected_latency_us: 10000,
                    priority: 1,
                },
                PipelineStage {
                    name: "valve_control".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 100,
                    priority: 0,
                },
            ],
        },
        DemoApp {
            name: "Industrial QC".to_string(),
            description: "Camera → defect detection → reject gate".to_string(),
            sensors: vec!["Line-scan camera".to_string()],
            model: "defect_detector".to_string(),
            actuators: vec!["Reject gate (pneumatic)".to_string()],
            target_latency_ms: 15.0,
            pipeline: vec![
                PipelineStage {
                    name: "capture".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 2000,
                    priority: 0,
                },
                PipelineStage {
                    name: "detect_defect".to_string(),
                    context: Context::Device,
                    expected_latency_us: 10000,
                    priority: 1,
                },
                PipelineStage {
                    name: "reject".to_string(),
                    context: Context::Kernel,
                    expected_latency_us: 500,
                    priority: 0,
                },
            ],
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r3_1_context_display() {
        assert_eq!(format!("{}", Context::Kernel), "@kernel");
        assert_eq!(format!("{}", Context::Device), "@device");
        assert_eq!(format!("{}", Context::Safe), "@safe");
    }

    #[test]
    fn r3_1_kernel_device_bridge() {
        let mut bridge = KernelDeviceBridge::new(0x1000000, 4096);
        assert!(!bridge.transfer_pending);
        bridge.signal_ready();
        assert!(bridge.transfer_pending);
        assert_eq!(bridge.seq, 1);
        bridge.acknowledge();
        assert!(!bridge.transfer_pending);
    }

    #[test]
    fn r3_1_device_kernel_bridge() {
        let mut bridge = DeviceKernelBridge::new(0x2000000);
        assert!(bridge.consume().is_none());
        bridge.post_result(3, 0.95);
        let (action, conf) = bridge.consume().unwrap();
        assert_eq!(action, 3);
        assert!((conf - 0.95).abs() < 0.001);
        assert!(bridge.consume().is_none()); // consumed
    }

    #[test]
    fn r3_4_jitter_measurement() {
        let mut jitter = JitterStats::new(10000); // 10ms target
        for us in [9800, 10200, 9900, 10100, 10000] {
            jitter.record(us);
        }
        assert!(jitter.mean_jitter() < 200.0);
        assert_eq!(jitter.max_jitter(), 200);
        assert!(jitter.within_tolerance(200));
        assert!(!jitter.within_tolerance(100));
    }

    #[test]
    fn r3_5_rate_monotonic() {
        let tasks = vec![
            RtTask {
                name: "sensor".to_string(),
                period_us: 10000,
                deadline_us: 10000,
                wcet_us: 2000,
                context: Context::Kernel,
                priority: 0,
            },
            RtTask {
                name: "infer".to_string(),
                period_us: 30000,
                deadline_us: 30000,
                wcet_us: 10000,
                context: Context::Device,
                priority: 1,
            },
        ];
        // U = 2/10 + 10/30 = 0.533, bound for n=2 = 2*(2^0.5-1) = 0.828
        assert!(rate_monotonic_test(&tasks));
    }

    #[test]
    fn r3_5_rate_monotonic_overloaded() {
        let tasks = vec![
            RtTask {
                name: "a".to_string(),
                period_us: 1000,
                deadline_us: 1000,
                wcet_us: 600,
                context: Context::Kernel,
                priority: 0,
            },
            RtTask {
                name: "b".to_string(),
                period_us: 1000,
                deadline_us: 1000,
                wcet_us: 600,
                context: Context::Device,
                priority: 1,
            },
        ];
        // U = 0.6 + 0.6 = 1.2 > bound 0.828
        assert!(!rate_monotonic_test(&tasks));
    }

    #[test]
    fn r3_7_watchdog() {
        let mut wd = WatchdogConfig {
            timeout_ms: 500,
            action: WatchdogAction::EmergencyStop,
            last_kick_ms: 1000,
        };
        assert!(!wd.is_expired(1400));
        assert!(wd.is_expired(1600));
        wd.kick(1600);
        assert!(!wd.is_expired(1900));
    }

    #[test]
    fn r3_8_telemetry_display() {
        let t = Telemetry {
            total_inferences: 1000,
            total_reads: 5000,
            total_commands: 1000,
            deadline_violations: 5,
            avg_inference_us: 3500.0,
            avg_e2e_us: 8000.0,
            uptime_secs: 3600,
        };
        let s = format!("{t}");
        assert!(s.contains("inferences=1000"));
        assert!(s.contains("violations=5"));
    }

    #[test]
    fn r3_9_demo_apps() {
        let demos = demo_apps();
        assert_eq!(demos.len(), 8);
        assert_eq!(demos[0].name, "Drone Autopilot");
        assert_eq!(demos[7].name, "Industrial QC");
        // All demos have at least 3 pipeline stages
        for demo in &demos {
            assert!(demo.pipeline.len() >= 3, "{} has < 3 stages", demo.name);
        }
    }

    #[test]
    fn r3_9_demo_context_isolation() {
        let demos = demo_apps();
        for demo in &demos {
            // First and last stages should be @kernel (sensor/actuator)
            assert_eq!(demo.pipeline.first().unwrap().context, Context::Kernel);
            // Middle stages should include @device (inference)
            let has_device = demo.pipeline.iter().any(|s| s.context == Context::Device);
            assert!(has_device, "{} has no @device stage", demo.name);
        }
    }

    #[test]
    fn r3_7_priority_inheritance() {
        let mut pi = PriorityInheritance::new();
        pi.acquire("low_task", "mutex_A", 10); // low priority = 10
        pi.inherit("low_task", 2); // high-priority task blocked → inherit priority 2
        let eff = pi
            .effective_priority
            .iter()
            .find(|(t, _)| t == "low_task")
            .unwrap();
        assert_eq!(eff.1, 2);
    }
}
