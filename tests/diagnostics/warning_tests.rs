#[path="../../source/runtime/src/diagnostic_log.rs"] mod diagnostic_log;
#[test] fn repeated_warning_is_bounded_and_counted() {
 let mut gate=diagnostic_log::WarningGate::default();
 assert_eq!(gate.observe(0),Some(0));
 for now in 1..10000 {assert_eq!(gate.observe(now),None);}
 assert_eq!(gate.observe(10000),Some(9999));
 assert_eq!(gate.observe(10001),None);
 assert_eq!(gate.observe(20000),Some(1));
}
