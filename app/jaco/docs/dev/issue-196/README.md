# Jaco：Issue #196 managed artifact wiring 与 crash reconciliation

## Root hub and ownership

- Plan ID：`issue-196`
- Root status：`Implemented locally`；agent动画首帧校验与root `T-11`基准已完成，`Done`受`RG-01/T-07/T-10`约束
- Root hub：[Issue #196 root plan](../../../../../docs/dev/issue-196/README.md)
- Owner：`app/jaco`
- Owner index：[Jaco 开发计划](../README.md)
- Assigned WP：`WP-301`
- Root contracts consumed：`D-06`–`D-11`、`C-03`–`C-04`、`G-01`、`ST-01`、`ERR-04`–`ERR-05`、`R-06`–`R-13`、`T-06/T-07`
- Owner-local IDs：`E/D/F/L/ST/G/R/T/WP-3xx`
- Owns：exact DatabaseTarget directory、SessionAgentPersistence batch adapter、NewAttachment app constructors、startup orphan reconciliation、app integration tests
- Does not own：OpenRouter/Rig/network/decode、DB transaction/schema、artifact ERR mapping、timeline rendering contract、live provider compatibility decision

## Owner implementation result（2026-08-31）

- `WP-301` 已实现；conversation `89/89` 及 generated reconciliation、session、Ready data dir、startup recovery、publication/reload 聚焦测试通过。
- `cargo check -p jaco --all-targets --all-features` 与最终 workspace clippy `-D warnings` 通过。
- 未修改生产 UI/locales/assets/manifest；`T-07` 手工 preview/save/restart/follow-up 留给 root 发布验收。

## Owner evidence

| E-ID | Current fact | Evidence | Consequence |
| --- | --- | --- | --- |
| `E-301` | `DatabaseResource.target.data_dir`是当前opened database的authoritative data root | `src/database.rs::{DatabaseTarget,DatabaseResource}` | artifact store与reconciliation必须从ready target读取，不能ambient resolve |
| `E-302` | `ready_agent_persistence`与`ready_executor`都绑定当前Ready session | `src/database.rs`、`database/session.rs` | runtime injection、DB batch和reconciliation使用同一session/lease |
| `E-303` | `SessionAgentPersistence`逐方法把agent port转发给FreshRepository | `src/database/session.rs` | C-03只增加同形adapter，无第二DB owner |
| `E-304` | managed dir与conversation ID validation已在conversation attachments模块实现 | `src/features/conversation/attachments.rs::{managed_attachment_dir,is_valid_managed_conversation_id}` | 继续作为app-local path authority并为agent计算concrete dir |
| `E-305` | production `conversation_data_dir`重新调用`paths::data_dir()`，test才读取DatabaseTarget | `src/features/conversation.rs::conversation_data_dir` | 收敛为统一`database::ready_data_dir`以消除target drift |
| `E-306` | user attachment preparation在DB前写managed files，DB失败后按path清理 | `src/features/conversation/attachments.rs`、`features/conversation.rs::send_conversation_message` | `NewAttachment.id`同步更新，但不把user flow迁入generated G-01 |
| `E-307` | runtime每次run构造AgentRuntime，持有active task/cancellation/observer | `src/features/conversation/runtime.rs::{prepare_active_run,run_agent_with_saved_provider}` | 每次run注入matching conversation ManagedArtifactStore；不加Store state |
| `E-308` | runtime create先执行interrupted-run recovery，recovery Ready前submission不可用 | `src/features/conversation/runtime.rs::{create,request_recovery,ensure_submission_available}` | orphan sweep串在同一recovery task后，无active run race |
| `E-309` | #195 resolver只允许GeneratedFile位于当前managed conversation root，并已有preview/save | `src/components/chat/detail/attachment_access.rs`、`detail/attachments.rs` | artifact成功后UI零改动；manual验收复用existing actions |
| `E-310` | data root有exclusive database lease，refresh会drain旧session | `src/database.rs::{DatabaseTargetLease,retire_failed_refresh}` | startup/retry sweep不会与另一Jaco process或旧DB job并发 |
| `E-311` | `ConversationCommitted`携带summary并更新catalog/recency；`ConversationTimelineChanged`只更新已打开timeline | `features/conversation/runtime.rs::handle_runtime_event`、`features/conversation/registry.rs::publish_changes` | generated batch必须消费agent的`ConversationCommitted`，integration test同时断言timeline与catalog |

## Owner-local decisions

| D-ID | Decision | Root authority / evidence | Consequence |
| --- | --- | --- | --- |
| `D-301` | 新`ready_data_dir`直接clone Ready resource target；send flow与runtime统一使用 | root `G-01`；`E-301/E-305` | prod/test/repair target一致 |
| `D-302` | app用existing validation/helper计算conversation dir，再构造`ManagedArtifactStore(conversation_id, dir)` | root `D-08`；`E-304/E-307` | agent不拼接ambient root或conversation path |
| `D-303` | 每次run都可注入store；只有agent generated predicate会使用，no image run不创建directory | root `D-04/D-05` | ordinary run filesystem behavior保持 |
| `D-304` | SessionAgentPersistence只调用repository C-03 batch并返回完整commit；SessionDatabaseExecutor另提供generated index只读wrapper；均受existing drain permit约束 | root `C-03/C-04`；`E-303` | agent仍是runtime publication owner，app recovery不扩展AgentPersistence |
| `D-305` | app所有`NewAttachment` producers显式`new_id()`；user names/path/content ordering保持 | root `D-06`；`E-306` | coordinatedsource break without behavior migration |
| `D-306` | generated reconciliation在interrupted-run recovery成功后执行，recovery Ready前结束 | root `G-01/ST-01`；`E-308/E-310` | scan与active writes无race，不新增task owner |
| `D-307` | reconciliation从DB generated index建立authority，只扫描root immediate conversation dirs和`.pending`/immediate files；final删除谓词要求`.jaco-generated-`保留前缀 | root `G-01` | 不递归；无前缀UUID、user attachment grammar和其他data files均不可达 |
| `D-308` | unsafe/malformed/symlink/ambiguous record/file全部保留并warn；只删除可证明generated且无matching DB authority的regular file | root `D-08/D-10`；`E-309` | cleanup favors no data loss and no path escape |
| `D-309` | DB index读取失败使runtime recovery失败；filesystem inspection/delete、missing root/reference只发safe degraded warning并允许Ready，且永不删DB row | root `ERR-04/05` | DB query remains hard gate；外部缺失/cleanup可由UI resolver和下次recovery继续暴露 |
| `D-310` | components/chat/detail、locales、assets无diff；manual只验证existing UI消费新records | root `D-11`；`E-309` | 不扩展UI scope |

## File and ownership tree

```text
app/jaco/
├── src/
│   ├── database.rs                              # F-301 [Modify] ready_data_dir accessor
│   ├── database/session.rs                      # F-302 [Modify] C-03 AgentPersistence adapter + generated index executor wrapper
│   ├── features/conversation.rs                 # F-303 [Modify] shared ready target + NewAttachment source break
│   ├── features/conversation/attachments.rs     # F-304 [Modify] caller IDs; retain canonical managed helpers/tests
│   ├── features/conversation/runtime.rs         # F-305 [Modify] per-run store injection + recovery sequencing/tests
│   └── features/conversation/generated_artifacts.rs # F-306 [Add] bounded non-recursive startup reconciliation/tests
└── docs/dev/
    ├── README.md                                # F-307 [Modify] owner index
    └── issue-196/README.md                      # F-308 [Add] this plan
```

Additional compile-only constructor update if required bythe source break：

```text
app/jaco/src/components/chat/detail.rs test fixtures  # F-309 [Modify] add explicit NewAttachment.id only
```

Explicit unchanged：

```text
app/jaco/src/components/chat/detail/{attachments.rs,attachment_access.rs,message.rs,timeline.rs}
app/jaco/locales/**
app/jaco/assets/**
app/jaco/build-assets/**
app/jaco/Cargo.toml
```

`WP-301`自身不改manifest/lock；聚合分支的预期lock metadata变化由root `DEP-01` / agent `WP-202`拥有。

## L-301：Ready target accessor

```rust
pub(crate) fn ready_data_dir(cx: &impl AppContext) -> jaco_db::Result<PathBuf>;
```

Rules：

1. Call existingshutdown guard。
2. Read`DatabaseStore` only whenoperation isexact Ready。
3. Return`resource.target.data_dir.clone()` fromthe sameReady resource that owns session。
4. Unavailable/refreshing/draining returnsInvariant；no fallback to`paths::data_dir()`。
5. Replace currentcfg-split`conversation_data_dir` and use accessor foruser attachment prep andruntime injection。

This does not expose database path/lease or create directories。

## L-302：Per-run store injection

`prepare_active_run` obtains in one synchronous availability window：

```text
persistence = ready_agent_persistence(cx)
data_dir    = ready_data_dir(cx)
provider    = ready_provider(...)
```

Then：

1. Validateconversation ID with existing helper。
2. Compute`managed_attachment_dir(&data_dir, &conversation_id)`。
3. Construct`ManagedArtifactStore::new(conversation_id.clone(), concrete_dir)`。
4. Move it withpersistence/request/provider intoexistingrun task。
5. Build`AgentRuntime::new(persistence).with_managed_artifact_store(store)` beforeMCP preparation/begin_run。

Failure tovalidate/obtain target follows existing submission/setup failure path and produces no provider request。No Entity/Store/Global stores thepath orartifact state。

Stop/shutdown continues tocancel current token andawait existingrun task；artifact work is nested inthat task。

## L-303：Session persistence adapter

`SessionAgentPersistence` addsroot C-03 method and imports：

```rust
async fn append_conversation_entries_with_attachments(
    &self,
    items: Vec<NewConversationEntryBatchItem>,
) -> jaco_db::Result<ConversationCommit<AppendedConversationEntryBatch>> {
    repository_call!(self, append_conversation_entries_with_attachments(items))
}
```

No file operation、cleanup、event publication或retry enters this adapter。`SessionDatabaseExecutor` draining permitcontinues tocover the entirerepository transaction。

Startup recovery使用app-local executor wrapper，不把全库index加入`AgentPersistence`：

```rust
pub(crate) async fn generated_file_attachments(
    &self,
) -> jaco_db::Result<Vec<AttachmentRecord>> {
    self.execute(|repository| repository.generated_file_attachments())
        .await
}
```

该方法位于`database/session.rs::SessionDatabaseExecutor`，只负责existing`execute` + drain permit下的只读repository call；无scan/delete/retry/publication。

## G-301：Startup reconciliation input

`request_recovery`在同一同步Ready窗口取得persistence、executor、data dir，再把三者捕获进existing recovery task：

```text
request_recovery task
  → AgentRuntime::recover_interrupted_runs()
  → executor.generated_file_attachments()
  → smol::unblock(reconcile_generated_artifacts(data_dir, records))
  → settle existing refresh::Operation
```

- interrupted runs finalize before files are classified, so no active/generated write remains。
- DB query error maps toexistingConversationRuntimeProblem andcurrent recovery failure flow。
- scan summary containsonly counts/categories；no rawrecords/paths。

## G-302：Non-recursive reconciliation algorithm

Expected DB authority map is keyed by`(conversation_id, attachment_id)` and retains normalized exact path/source/storage/kind facts。

Filesystem algorithm：

1. Root is`data_dir/attachments`。Missing root + emptyDB index isclean success；missing root + nonempty index preservesall rows，emits`missing_managed_root` + count warning andreturnsdegraded success/Ready。
2. `symlink_metadata(root)`；symlink/non-directory → warning, zero mutation。
3. Iterate immediatechildren only；accept non-symlink directories whosebasename passesexistingconversation ID validator。
4. For eachconversation dir：
   - `.pending` must bea non-symlink directory；iterate immediate regularfiles only。
   - delete only names matching`{uuid}.part` and noactive run exists（guaranteed byG-301）。
   - iterate immediate final regularfiles matchingthe app-reserved`.jaco-generated-{uuid}.{png|jpg|jpeg|gif|webp}`grammar；unprefixed`{uuid}.{ext}`、composer andlegacy files never match。
5. Keep final iffDB has sameconversation/ID andrecord isImage + GeneratedFile + GeneratedFile source andbothrecord locators canonicalize tothis exact file insideconversation root。
6. NoDB row → delete；malformed/mismatched/duplicate DB facts → preserve andwarning。
7. Afterscan，any otherwise well-formedDB authority whose exactregular file wasnot observed remainsunchanged andemits aggregated`missing_generated_file` warning；no row repair/delete occurs。
8. Never follow symlink、never recurse、never delete directory、never inspect/deletecomposer filename grammar。

Delete/inspection/missing-reference warnings aggregate bysafe category/count only and allow recoveryReady；nextstartup/retry repeats。A deletedorphan has noDB row and therefore no UI/history authority。Only generated-index DB query failure followsD-309 hard recovery failure。

## L-304：Existing NewAttachment producers

Every app constructor adds`id: new_id()` beforestorage path/DB call。For user attachments：

- DB record ID remains independent fromcomposer local_id/storage filename。
- existingcontent part append still occurs inDB helper。
- LocalFile/GeneratedImage composer source、MIME/name/path/metadata andcleanup list areunchanged。
- no generated provider file is routed throughcomposer types。

Test fixture constructors use explicit deterministic/new IDs as appropriate；no helper hides thefield。

## UI integration boundary

No production UI edit。Manual/integration assertions consume persisted facts：

- Assistant Message hasordered`ContentPart::Image`。
- attachment record isImage/GeneratedFile withsafe managed path。
- current#195 projection renders thumbnail andpreview dialog。
- Save copy/open/reveal availability derives fromexistingresolver。
- reload/restart readsDB andsamefile；follow-up history isagent-owned。

Any UI defect discovered here is reported as a deviation; implementation must not redesign cards/actions/locales underWP-301 without user scope change。

## Compatibility and rollback

| Surface | Result |
| --- | --- |
| database target | stronger consistency：prod/test use sameReady target；no path migration |
| existing user attachments | onlycaller-assigned record ID changes internally；file/content/UI behaviorsame |
| runtime state | no newentity/task/operation；recovery gets one extra awaited phase |
| existing generated/user files | reconciliation only recognizesnew `.jaco-generated-{attachment-id}.{ext}` grammar；unprefixed UUID、composer andall otherfiles untouched |
| malformed/symlink paths | fail-safe preserve/no mutation；future provider generation fails inagent rather than escaping root |
| UI/i18n/assets | no change |
| rollback | persistedartifacts remain readable；orphan sweep stops；no schema/config rollback |

## Owner requirements

| R-ID | Requirement |
| --- | --- |
| `R-301` | Every run and user attachment flow uses exactReady DatabaseTarget data dir with noambient fallback. |
| `R-302` | ManagedArtifactStore receives thevalidated concrete directory for the sameconversation/session. |
| `R-303` | Session persistence adapter forwards oneC-03 transaction；executor wrapper forwards onegenerated-index read；both useexistingdrain permit and do nothing else. |
| `R-304` | All app NewAttachment constructors supply explicit IDs without changinguser semantics. |
| `R-305` | Reconciliation runs afterinterrupted-run recovery and before runtimeReady, with noactive-run race. |
| `R-306` | G-302 deletes onlyproven`.jaco-generated-`orphans/pendingfiles，preserves everyunprefixed UUID/ambiguous/unsafe/user path andnever deletes missing-reference DB rows. |
| `R-307` | DB read errors vsfilesystem cleanup/missing-reference outcomes followD-309 exactly and emit safe category/count diagnostics. |
| `R-308` | Existing Assistant image UI/reload/preview/save succeeds；`ConversationCommitted` updatescatalog summary/recency withzero production UI/i18n/assets changes. |
| `R-309` | app manifest/package/workflow remain unchanged；WP-301 makes no lock edit. |

## WP-301：Wire managed storage and recovery

**Prerequisites**

- `WP-101` records/repository/port types complete。
- `WP-202` exportsfinal`ManagedArtifactStore` contract。

**Implementation sequence**

1. AddL-301 and replacecfg-split ambientdata-dir reads。
2. ImplementL-303 Session persistence batch adapter、executor index wrapper andL-304 constructor updates。
3. Injectstore perL-302 without changingactive-run state/task lifecycle。
4. Addgenerated artifacts module andG-301/G-302 reconciliation。
5. Addfocused app/database-target/recovery tests。
6. Manually verifyexisting #195 UI matrix；do not editUI unless root plan is revised。

**Exit criteria**

- R-301–R-309 pass；root may executeT-07/RG-01/aggregate gates。

## Validation

| T-ID | Required test |
| --- | --- |
| `T-301` | Ready/Unavailable/Refreshing/shutdown target accessor and noambient fallback. |
| `T-302` | production-shaped run receivesmatchingconversation ID + managed dir; ordinary run does not create dir. |
| `T-303` | Session batch adapter returnsauthoritative ordered commit；executor index wrapper returnsrecords；both respectdraining rejection. |
| `T-304` | user attachment send/create preservespath/content/order/cleanup withcaller IDs. |
| `T-305` | startup clearsvalid pending andunreferenced`.jaco-generated-{uuid}.{ext}`final，keepsreferenced exact files andunprefixed UUID/composer/legacy files；missing root with emptyindex isclean，withrows iswarning/Ready andpreservesrows. |
| `T-306` | symlink root/dir/file、invalidconversation dir、malformed/duplicate/mismatched record andcomposer files areuntouched. |
| `T-307` | DB index error failsrecovery；delete/inspection/missing referenced-file cases warn withsafe counts，preserverows andrecovery can settleReady. |
| `T-308` | repeated recovery isidempotent；two validconversation dirs do notcross-delete. |
| `T-309` | app integration reloadsassistant generated attachment fromDB，existingresolver accepts exact path，and`ConversationCommitted` refreshestimeline + catalog summary/recency. |
| `T-310` | manual thumbnail/preview/save/restart/follow-up + ordinary streaming matrix. |

Focused commands during implementation：

```text
cargo test -p jaco database
cargo test -p jaco conversation
cargo check -p jaco --all-targets --all-features
```

Root ownsfinal fmt/workspace/clippy/live/CI gates。

## Implementer/Auditor reread

- [ ] `ready_data_dir` and session/persistence are read from the same Ready resource.
- [ ] No production code calls ambient `paths::data_dir()` for conversation artifacts after L-301.
- [ ] Store injection adds no Entity/Store/Global/Operation or detached task.
- [ ] Session persistence/executor wrappers haveexact L-303 signatures，share existingdrain semantics and contain no filesystem, cleanup, retry or publication logic.
- [ ] Reconciliation starts only after interrupted runs finish and before submissions unblock.
- [ ] Scan is bounded to two explicit directory levels, rejects symlinks and matches only the exact`.jaco-generated-{uuid}.{ext}`final grammar；unprefixed UUID files are preserved.
- [ ] Ambiguous/malformed data is preserved; only absence of DB authority permits deletion.
- [ ] Missing root/reference with DB authority is observable bysafe warning，preservesrows and is notreported asclean success.
- [ ] User composer files and all unrelated data are unreachable by the delete predicate.
- [ ] Production timeline/locales/assets/manifest/packaging/workflow have no diff.
