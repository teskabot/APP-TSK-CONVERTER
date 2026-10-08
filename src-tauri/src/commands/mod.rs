pub mod conflicts;
pub mod convert;
pub mod scan;
pub mod system;

use std::{path::PathBuf, sync::Mutex};
use std::sync::{atomic::AtomicBool, Arc};

#[derive(Default)]
pub struct CancelFlag(pub Arc<AtomicBool>);

#[derive(Default)]
pub struct LastOutputDirectory(pub Mutex<Option<PathBuf>>);
