use serde::{Serialize, Serializer, ser::SerializeSeq};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    rc::Rc,
};

const MAX_RETAINED_ROWS: usize = 256;
const MAX_RETAINED_COLUMNS: usize = 128;
const MAX_RETAINED_RESULT_CAPACITY: usize = 4096;

use crate::{
    utils::errors::{TextForgeError, TextForgeErrorCode},
    watchers::ExecutionWindows::All,
};

pub mod default_watchers;
mod shared_statistics;
use shared_statistics::{SharedMetric, Statistics};

use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct WatcherContext {
    pub current: Arc<str>,
    pub before: Arc<str>,
    pub after: Option<Arc<str>>,
    pub instruction: Arc<str>,
}

impl WatcherContext {
    /// Constrói um `WatcherContext` a partir de seus quatro campos.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use textforge::watchers::WatcherContext;
    /// use std::sync::Arc;
    ///
    /// let ctx = WatcherContext::new(
    ///     "banana!",
    ///     "banana",
    ///     Some("BANANA!"),
    ///     "add_to_end",
    /// );
    ///
    /// assert_eq!(ctx.current.as_ref(), "banana!");
    /// assert_eq!(ctx.after.as_deref(), Some("BANANA!"));
    /// ```
    pub fn new<C, B, A, I>(current: C, before: B, after: Option<A>, instruction: I) -> Self
    where
        C: Into<Arc<str>>,
        B: Into<Arc<str>>,
        A: Into<Arc<str>>,
        I: Into<Arc<str>>,
    {
        WatcherContext {
            current: current.into(),
            before: before.into(),
            after: after.map(Into::into),
            instruction: instruction.into(),
        }
    }
}

/// Decide quando os watchers de uma [`WatcherList`] são executados em
/// relação às instruções do pipeline.
///
/// Atualmente só `All` (todo passo) está implementado; as demais
/// variantes planejadas ficam comentadas até terem uma execução real
/// em [`WatcherList::run_watchers`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionWindows {
    /// O watcher roda em toda instrução do pipeline.
    All = 0,
    // Every(usize),
    // After(usize),
    // Before(usize),
}

/// Registro de watchers e agenda de execução para um pipeline ATP.
///
/// Um `WatcherList` guarda três coisas separadas:
/// - `watchers`: funções nomeadas que recebem um [`WatcherContext`] e
///   retornam uma `String` (via [`set_watcher`](WatcherList::set_watcher));
/// - `schedule`: em quais momentos cada watcher registrado deve rodar
///   (via [`schedule_watcher`](WatcherList::schedule_watcher));
/// - `result`: o relatório acumulado, indexado por número do passo do
///   pipeline (populado por [`run_watchers`](WatcherList::run_watchers),
///   exportável via [`to_json`](WatcherList::to_json)).
///
/// `watchers` e `schedule` sobrevivem a [`reset`](WatcherList::reset) —
/// só `result` e `counter` são limpos — pra permitir reusar a mesma
/// instância em múltiplas execuções do pipeline sem reregistrar tudo.
///
/// # Examples
///
/// ```rust
/// use textforge::watchers::{WatcherList, ExecutionWindows, WatcherContext};
///
/// let mut watchers = WatcherList::default();
///
/// watchers.set_watcher("current_len", |ctx: WatcherContext| {
///     ctx.current.len().to_string()
/// });
/// watchers.schedule_watcher("current_len", ExecutionWindows::All)?;
///
/// let ctx = WatcherContext::new(
///     "banana".to_string(),
///     "".to_string(),
///     None::<&str>,
///     "add_to_end".to_string(),
/// );
/// watchers.run_watchers(ctx)?;
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Default)]
pub struct WatcherList {
    result: Vec<HashMap<Arc<str>, String>>,
    watchers: HashMap<Arc<str>, RegisteredWatcher>,
    schedule: Vec<ScheduledWatcher>,
    spare_rows: Vec<HashMap<Arc<str>, String>>,
    counter: u64,
}

impl WatcherList {
    /// Cria uma `WatcherList` vazia. Alias de [`Default::default`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear results and reset the step counter, retaining a bounded row pool.
    /// At most 256 empty maps with capacity for at most 128 entries are retained. Result-value
    /// strings are dropped; unusually large outer vectors are released.
    ///
    /// Watchers registrados e o agendamento (`watchers`/`schedule`) **não**
    /// são afetados — a instância continua pronta pra rodar outro pipeline
    /// sem precisar reregistrar nada.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use textforge::watchers::{WatcherList, ExecutionWindows, WatcherContext};
    ///
    /// let mut watchers = WatcherList::default();
    /// watchers.set_watcher("len", |ctx: WatcherContext| ctx.current.len().to_string());
    /// watchers.schedule_watcher("len", ExecutionWindows::All)?;
    ///
    /// let ctx = WatcherContext::new("ab", "", None::<&str>, "noop");
    /// watchers.run_watchers(ctx)?;
    ///
    /// watchers.reset();
    /// // "len" continua registrado e agendado após o reset.
    /// let ctx2 = WatcherContext::new("abc", "", None::<&str>, "noop");
    /// watchers.run_watchers(ctx2)?;
    /// # Ok::<(), textforge::utils::errors::TextForgeError>(())
    /// ```
    pub fn reset(&mut self) {
        self.counter = 0;
        for mut row in self.result.drain(..) {
            if self.spare_rows.len() < MAX_RETAINED_ROWS && row.capacity() <= MAX_RETAINED_COLUMNS {
                row.clear();
                self.spare_rows.push(row);
            }
        }
        if self.result.capacity() > MAX_RETAINED_RESULT_CAPACITY {
            self.result = Vec::new();
        }
    }

    /// Registra (ou substitui) um watcher sob `watcher_name`.
    ///
    /// Registrar um watcher **não** o agenda pra rodar — é preciso chamar
    /// [`schedule_watcher`](WatcherList::schedule_watcher) separadamente.
    /// Chamar `set_watcher` de novo com um nome já usado substitui a
    /// função anterior silenciosamente; o agendamento existente pra esse
    /// nome (se houver) não é afetado.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use textforge::watchers::{WatcherList, WatcherContext};
    ///
    /// let mut watchers = WatcherList::default();
    /// watchers.set_watcher("is_empty", |ctx: WatcherContext| ctx.current.is_empty().to_string());
    /// ```
    pub fn set_watcher<F>(&mut self, watcher_name: &'static str, watcher: F)
    where
        F: Fn(WatcherContext) -> String + 'static,
    {
        let metric = SharedMetric::identify::<F>();
        self.register(
            Arc::from(watcher_name),
            Rc::new(move |context| Ok(watcher(context))),
            metric,
        );
    }

    pub(crate) fn set_owned_watcher(
        &mut self,
        name: String,
        callback: impl Fn(WatcherContext) -> Result<String, TextForgeError> + 'static,
    ) {
        self.register(Arc::from(name), Rc::new(callback), None);
    }

    fn register(
        &mut self,
        name: Arc<str>,
        callback: WatcherCallback,
        metric: Option<SharedMetric>,
    ) {
        self.watchers.insert(
            name.clone(),
            RegisteredWatcher {
                callback: Rc::clone(&callback),
                metric,
            },
        );
        if let Ok(index) = self
            .schedule
            .binary_search_by(|entry| entry.name.as_ref().cmp(name.as_ref()))
        {
            self.schedule[index].callback = callback;
            self.schedule[index].metric = metric;
        }
    }

    /// Agenda um watcher já registrado pra rodar segundo `when`.
    ///
    /// # Errors
    ///
    /// Retorna `Err` se `watcher_name` não corresponder a nenhum watcher
    /// registrado via [`set_watcher`](WatcherList::set_watcher).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use textforge::watchers::{WatcherList, ExecutionWindows, WatcherContext};
    ///
    /// let mut watchers = WatcherList::default();
    /// watchers.set_watcher("len", |ctx: WatcherContext| ctx.current.len().to_string());
    ///
    /// assert!(watchers.schedule_watcher("len", ExecutionWindows::All).is_ok());
    /// assert!(watchers.schedule_watcher("nao_existe", ExecutionWindows::All).is_err());
    /// ```
    pub fn schedule_watcher(
        &mut self,
        watcher_name: &'static str,
        when: ExecutionWindows,
    ) -> Result<(), TextForgeError> {
        self.schedule_owned_watcher(watcher_name, when)
    }

    pub(crate) fn schedule_owned_watcher(
        &mut self,
        watcher_name: &str,
        when: ExecutionWindows,
    ) -> Result<(), TextForgeError> {
        let callback = self.watchers.get(watcher_name).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::WatcherNotFoundError(
                    format!("Watcher {watcher_name} not found").into(),
                ),
                "WatcherList.schedule_watcher",
                watcher_name.to_string(),
            )
        })?;
        match self
            .schedule
            .binary_search_by(|entry| entry.name.as_ref().cmp(watcher_name))
        {
            Ok(index) => {
                self.schedule[index].when = when;
            }
            Err(index) => self.schedule.insert(
                index,
                ScheduledWatcher {
                    name: Arc::from(watcher_name),
                    when,
                    callback: Rc::clone(&callback.callback),
                    metric: callback.metric,
                },
            ),
        }
        Ok(())
    }

    /// Executa todos os watchers agendados contra `input`, armazenando
    /// cada resultado sob o passo atual (`counter`) no relatório interno,
    /// e então incrementa `counter`.
    ///
    /// Pensado pra ser chamado uma vez por instrução dentro de um loop de
    /// processamento (ver `TextForgeProcessor::process_all_with_watchers`), na
    /// mesma ordem em que as instruções do pipeline rodam — a correção do
    /// relatório depende dessa ordem, já que `counter` não é derivado de
    /// `input`, só da sequência de chamadas.
    ///
    /// # Errors
    ///
    /// Propaga qualquer erro do watcher subjacente (hoje, apenas o caso
    /// interno de inconsistência entre `schedule` e `watchers`, que não é
    /// alcançável através da API pública atual).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use textforge::watchers::{WatcherList, ExecutionWindows, WatcherContext};
    ///
    /// let mut watchers = WatcherList::default();
    /// watchers.set_watcher("len", |ctx: WatcherContext| ctx.current.len().to_string());
    /// watchers.schedule_watcher("len", ExecutionWindows::All)?;
    ///
    /// let ctx = WatcherContext::new("banana", "", None::<&str>, "add_to_end");
    /// watchers.run_watchers(ctx)?;
    /// # Ok::<(), textforge::utils::errors::TextForgeError>(())
    /// ```
    pub fn run_watchers(&mut self, input: WatcherContext) -> Result<(), TextForgeError> {
        let next = self
            .counter
            .checked_add(1)
            .ok_or_else(|| report_error("Watcher step counter overflow"))?;
        if self.schedule.is_empty() {
            self.counter = next;
            return Ok(());
        }
        self.result.try_reserve(1).map_err(report_error)?;
        let mut row = self.spare_rows.pop().unwrap_or_default();
        row.try_reserve(self.schedule.len()).map_err(report_error)?;
        let (current_mask, before_mask) = self
            .schedule
            .iter()
            .filter_map(|entry| entry.metric)
            .fold((0, 0), |(current, before), metric| {
                (
                    current | metric.current_mask(),
                    before | metric.before_mask(),
                )
            });
        let current_statistics = Statistics::compute(&input.current, current_mask);
        let before_statistics = Statistics::compute(&input.before, before_mask);
        for entry in &self.schedule {
            match entry.when {
                All => {
                    let value = match entry.metric {
                        Some(metric) => metric.evaluate(&current_statistics, &before_statistics),
                        None => (entry.callback)(input.clone())?,
                    };
                    row.insert(entry.name.clone(), value);
                }
            }
        }
        self.result.push(row);
        self.counter = next;
        Ok(())
    }

    /// Serializa o relatório acumulado como JSON formatado e escreve em
    /// `filename`, sobrescrevendo o arquivo se já existir.
    ///
    /// The format is `[ { "watcher_name": "value", ... }, ... ]`.
    /// Each observed step is one array entry with watcher names as keys.
    /// Keys and callback execution order are lexicographic by watcher name.
    /// The destination is replaced only after serialization and flushing succeed.
    /// Atomic replacement protects against partial writes; it does not promise
    /// persistence across power loss. Existing destination permissions are not copied.
    ///
    /// # Errors
    ///
    /// Retorna `Err` se a serialização falhar, ou se o arquivo não puder
    /// ser aberto/escrito.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use textforge::watchers::{WatcherList, ExecutionWindows, WatcherContext};
    /// use std::env::temp_dir;
    ///
    /// let mut watchers = WatcherList::default();
    /// watchers.set_watcher("len", |ctx: WatcherContext| ctx.current.len().to_string());
    /// watchers.schedule_watcher("len", ExecutionWindows::All)?;
    ///
    /// let ctx = WatcherContext::new("banana", "", None::<&str>, "add_to_end");
    /// watchers.run_watchers(ctx)?;
    ///
    /// let path = temp_dir().join("textforge_watcherlist_doctest.json");
    /// watchers.to_json(&path)?;
    /// assert!(path.exists());
    /// # std::fs::remove_file(&path).ok();
    /// # Ok::<(), textforge::utils::errors::TextForgeError>(())
    /// ```
    pub fn to_json(&self, filename: &Path) -> Result<(), TextForgeError> {
        publish_report(filename, |writer| self.write_json(writer))
    }

    fn write_json(&self, writer: &mut dyn Write) -> Result<(), TextForgeError> {
        serde_json::to_writer_pretty(&mut *writer, &(ReportView { rows: &self.result })).map_err(
            |error| {
                let code = if error.is_io() {
                    TextForgeErrorCode::FileWritingError(error.to_string().into())
                } else {
                    TextForgeErrorCode::SerializationError(error.to_string().into())
                };
                TextForgeError::new(code, "WatcherList.to_json", "report serialization")
            },
        )?;
        writer.flush().map_err(write_error)
    }
}

fn report_error(error: impl std::fmt::Display) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::ValidationError(error.to_string().into()),
        "WatcherList",
        "report allocation or counter",
    )
}

fn write_error(error: std::io::Error) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::FileWritingError(error.to_string().into()),
        "WatcherList.to_json",
        "report output",
    )
}

/// A shared callback keeps scheduled entries synchronized with replacements.
///
/// ```
/// use textforge::watchers::{WatcherList, ExecutionWindows};
/// let mut list = WatcherList::new();
/// list.set_watcher("bytes", |ctx| ctx.current.len().to_string());
/// list.schedule_watcher("bytes", ExecutionWindows::All)?;
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
type WatcherCallback = Rc<dyn Fn(WatcherContext) -> Result<String, TextForgeError>>;

/// Registered callbacks retain exact built-in identity across scheduling.
///
/// ```
/// use textforge::watchers::{WatcherList, default_watchers};
/// let mut list = WatcherList::new();
/// list.set_watcher("words", default_watchers::word_count);
/// list.set_watcher("words", |_| "custom".to_string());
/// ```
struct RegisteredWatcher {
    callback: WatcherCallback,
    metric: Option<SharedMetric>,
}

/// One prepared callback, avoiding schedule copies and registry lookups per step.
///
/// ```
/// use textforge::watchers::{WatcherList, ExecutionWindows};
/// let mut list = WatcherList::new();
/// list.set_watcher("bytes", |ctx| ctx.current.len().to_string());
/// list.schedule_watcher("bytes", ExecutionWindows::All)?;
/// list.set_watcher("bytes", |_| "replacement".to_string());
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
struct ScheduledWatcher {
    name: Arc<str>,
    when: ExecutionWindows,
    callback: WatcherCallback,
    metric: Option<SharedMetric>,
}

/// A borrowed serialization view sorts one row at a time without cloning values.
///
/// ```
/// use textforge::watchers::WatcherList;
/// let list = WatcherList::new();
/// let path = std::env::temp_dir().join(format!("textforge-report-{}.json", uuid::Uuid::new_v4()));
/// list.to_json(&path)?;
/// assert_eq!(std::fs::read_to_string(&path)?, "[]");
/// std::fs::remove_file(path)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Serialize)]
#[serde(transparent)]
struct ReportView<'a> {
    #[serde(serialize_with = "serialize_rows")]
    rows: &'a [HashMap<Arc<str>, String>],
}

fn serialize_rows<S: Serializer>(
    rows: &[HashMap<Arc<str>, String>],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut sequence = serializer.serialize_seq(Some(rows.len()))?;
    for row in rows {
        let ordered: BTreeMap<_, _> = row
            .iter()
            .map(|(name, value)| (name.as_ref(), value))
            .collect();
        sequence.serialize_element(&ordered)?;
    }
    sequence.end()
}

/// Remove an unpublished temporary report on errors or unwinding.
///
/// ```
/// use textforge::watchers::WatcherList;
/// let list = WatcherList::new();
/// let path = std::env::temp_dir().join(format!("textforge-report-{}.json", uuid::Uuid::new_v4()));
/// list.to_json(&path)?;
/// std::fs::remove_file(path)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
struct PendingReport {
    path: PathBuf,
}

impl Drop for PendingReport {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn publish_report(
    path: &Path,
    emit: impl FnOnce(&mut dyn Write) -> Result<(), TextForgeError>,
) -> Result<(), TextForgeError> {
    crate::utils::validations::prepare_output_path(path, "WatcherList.to_json")?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = parent.join(format!(".textforge-report-{}.tmp", uuid::Uuid::new_v4()));
    let file: File = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| {
            TextForgeError::new(
                TextForgeErrorCode::FileOpeningError(error.to_string().into()),
                "WatcherList.to_json",
                temporary.display().to_string(),
            )
        })?;
    let pending = PendingReport { path: temporary };
    let mut writer = BufWriter::with_capacity(64 * 1024, file);
    emit(&mut writer)?;
    writer.flush().map_err(write_error)?;
    drop(writer);
    std::fs::rename(&pending.path, path).map_err(write_error)
}

#[cfg(all(test, feature = "test_access"))]
mod test;
