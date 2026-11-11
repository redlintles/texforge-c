//! Safe whole-document reads and caller-owned batch scheduling.
use std::{fs::File, io::Read, path::Path};

use super::{TextForgeProcessor, TextForgeProcessorMethods};
use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

/// Read a document once, then transfer its allocation into a validated UTF-8 string.
/// File's `read_to_end` uses size information when available; wrapping it in a
/// BufReader would add another buffer without helping this whole-file read.
pub(super) fn read_utf8_file(
    path: &Path,
    location: &'static str,
) -> Result<String, TextForgeError> {
    let mut file = File::open(path).map_err(|error| {
        TextForgeError::new(
            TextForgeErrorCode::FileOpeningError(error.to_string().into()),
            location,
            format!("{}: {error}", path.display()),
        )
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|error| {
        TextForgeError::new(
            TextForgeErrorCode::FileReadingError(error.to_string().into()),
            location,
            format!("{}: {error}", path.display()),
        )
    })?;
    String::from_utf8(bytes).map_err(|error| {
        TextForgeError::new(
            TextForgeErrorCode::InvalidUtf8Error(error.utf8_error().to_string().into()),
            location,
            path.to_string_lossy().into_owned(),
        )
    })
}

impl TextForgeProcessor {
    /// Execute a batch inside a reusable, caller-owned Rayon thread pool.
    ///
    /// Results retain task order, including individual errors. This also applies
    /// with `buffered` enabled. Reuse the pool across calls to avoid thread creation
    /// per batch. The worker count limits active tasks, not bytes per document;
    /// transforms can still materialize or expand entire documents.
    ///
    /// ```
    /// use textforge::api::processor::TextForgeProcessor;
    /// let processor = TextForgeProcessor::new();
    /// let pool = rayon::ThreadPoolBuilder::new().num_threads(2).build()?;
    /// let results = processor.process_batch_in_pool(&pool, Vec::new());
    /// assert!(results.is_empty());
    /// // Keep `pool` for subsequent batches; the global Rayon pool is unchanged.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn process_batch_in_pool(
        &self,
        pool: &rayon::ThreadPool,
        tasks: Vec<(&Path, &str, &Path)>,
    ) -> Vec<Result<(), TextForgeError>> {
        pool.install(|| self.process_batch(tasks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        context::execution_context::GlobalExecutionContext,
        parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
        tokens::{InstructionMethods, instructions::null::Null},
    };
    use std::borrow::Cow;

    /// Test instruction observing the pool used by the actual batch worker.
    #[derive(Clone, Default)]
    struct PoolWidth(Null);

    impl InstructionMethods for PoolWidth {
        fn to_textforge_line(&self) -> Cow<'static, str> {
            self.0.to_textforge_line()
        }
        fn get_string_repr(&self) -> &'static str {
            "null"
        }
        fn get_params(&self) -> &Vec<TextForgeParamTypes> {
            self.0.get_params()
        }
        fn set_params(&mut self, _params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
            Ok(())
        }
        fn transform<'a>(
            &self,
            _input: Cow<'a, str>,
            _context: Option<&mut GlobalExecutionContext>,
        ) -> Result<Cow<'a, str>, TextForgeError> {
            Ok(Cow::Owned(rayon::current_num_threads().to_string()))
        }
        #[cfg(feature = "bytecode")]
        fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
            self.0.to_bytecode()
        }
        #[cfg(feature = "bytecode")]
        fn get_opcode(&self) -> u32 {
            self.0.get_opcode()
        }
    }

    #[test]
    fn actual_instructions_execute_inside_the_supplied_pool() {
        let directory =
            std::env::temp_dir().join(format!("textforge-pool-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let input = directory.join("input");
        let output = directory.join("output");
        std::fs::write(&input, "document").unwrap();
        let mut processor = TextForgeProcessor::new();
        let id = processor.add_transform(vec![TokenWrapper::new(
            Box::new(PoolWidth::default()),
            None,
        )]);
        for threads in [1, 3] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            for _ in 0..2 {
                let results = processor.process_batch_in_pool(&pool, vec![(&input, &id, &output)]);
                assert!(results[0].is_ok(), "{:?}", results[0]);
                assert_eq!(
                    std::fs::read_to_string(&output).unwrap(),
                    threads.to_string()
                );
            }
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
