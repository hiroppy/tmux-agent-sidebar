# TODO: バックグラウンドシェル実行中の状態検知

## 問題

Claude Code が `run_in_background: true` で Bash コマンドを実行した後、モデルの応答完了で `stop` フックが発火し sidebar は "idle" と表示する。しかし実際にはバックグラウンドプロセスがまだ実行中。

## 方針

`activity-log` フックで `Bash` ツールの `run_in_background: true` を検知し、tmux pane option `@pane_bg_shells` にフラグを立てる。`stop` イベント時にこのフラグがあれば "idle" ではなく "background" ステータスを設定する。

バックグラウンドプロセス完了後に `activity-log` が発火すると、既存の再活性化ロジック（`current_status != "running"`）が "background" を検知し "running" に復帰。

## 状態遷移フロー

```
user-prompt-submit → Running (bg_shells クリア)
    ↓
activity-log (Bash, bg=true) → Running (bg_shells=1)
    ↓
stop → Background (bg_shells=1, started_at 保持)
    ↓
[バックグラウンドプロセス完了]
    ↓
activity-log (任意ツール) → Running (bg_shells クリア)
    ↓
stop → Idle (bg_shells=0)
```

## 変更ファイル

### `src/tmux.rs`
- `PaneStatus` enum に `Background` バリアント追加
- `from_str`: `"background"` → `Background`
- `icon()`: `Background` → `"◎"`
- `pane_format` に `@pane_bg_shells` フィールド追加

### `src/cli/hook.rs`
- **`handle_activity_log`**:
  - `tool_name == "Bash"` かつ `tool_input["run_in_background"] == true` → `set @pane_bg_shells "1"`
  - 再活性化ロジック（L334）内で `unset @pane_bg_shells` を追加
- **`Stop` ハンドラ (L240-255)**:
  - `@pane_bg_shells` 確認、フラグあり → `set_status(pane, "background")` + `started_at` 保持
  - フラグなし → 既存動作（`clear_run_state` + `set_status(pane, "idle")`）
- **`UserPromptSubmit`**: `unset @pane_bg_shells`
- **`clear_all_meta`**: `@pane_bg_shells` を追加

### `src/ui/panes.rs`
- `running_icon_for`: `Background` → 固定アイコン `"◎"` + running 系の色（少し暗め）
- 詳細表示: `"  Background tasks…"` テキスト
- `is_active_status` に `Background` 追加

### `src/ui/colors.rs`
- `status_color` に `Background` 追加

### `src/state.rs`
- `StatusFilter::matches`: `Running` に `Background` もマッチ
- `status_counts`: `Background` を `running` にカウント

### テスト
- `from_str("background")` テスト
- activity-log で bg_shells 設定/クリアのテスト
- Stop 時の分岐テスト
- status_counts テスト
- スナップショットテスト更新
