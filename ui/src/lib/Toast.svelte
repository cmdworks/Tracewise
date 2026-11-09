<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { ToastMessage } from "./types";

  interface Props {
    toasts: ToastMessage[];
    ondismiss: (id: string) => void;
    onredo?: (undoId: string, repo?: string) => void;
  }

  let { toasts, ondismiss, onredo }: Props = $props();
</script>

<div class="toast-stack" role="region" aria-label="Notifications">
  {#each toasts as t (t.id)}
    <div class="toast-item toast-{t.type}">
      <div class="toast-icon">
        {#if t.type === "success" || t.type === "undo"}
          <Icon name="check" size={15} />
        {:else if t.type === "error"}
          <Icon name="alert" size={15} />
        {:else}
          <Icon name="info" size={15} />
        {/if}
      </div>

      <div class="toast-content">
        {#if t.title}
          <div class="toast-title font-mono">{t.title}</div>
        {/if}
        <div class="toast-message font-mono">{t.message}</div>
      </div>

      <div class="toast-actions">
        {#if t.undoId && onredo}
          <button
            type="button"
            class="toast-btn toast-btn-accent font-mono"
            onclick={() => onredo && t.undoId && onredo(t.undoId, t.repo)}
          >
            <Icon name="redo" size={12} />
            <span>Redo</span>
          </button>
        {/if}
        <button
          type="button"
          class="toast-close"
          aria-label="Dismiss notification"
          onclick={() => ondismiss(t.id)}
        >
          <Icon name="x" size={13} />
        </button>
      </div>
    </div>
  {/each}
</div>

<style>
  .toast-stack {
    position: fixed;
    bottom: 20px;
    right: 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: var(--z-toast, 60);
    max-width: 420px;
    pointer-events: none;
  }

  .toast-item {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    background: var(--card-elevated, #161926);
    border: 1px solid var(--border-strong, rgba(255, 255, 255, 0.16));
    border-radius: 8px;
    box-shadow: 0 16px 36px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.05);
    color: var(--foreground, #f3f5f8);
    animation: toastSlide 0.16s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .toast-success,
  .toast-undo {
    border-color: rgba(34, 197, 94, 0.45);
    background: #0c1813;
  }

  .toast-success .toast-icon,
  .toast-undo .toast-icon {
    color: var(--accent, #22c55e);
  }

  .toast-error {
    border-color: rgba(239, 68, 68, 0.45);
    background: #1a0f12;
  }

  .toast-error .toast-icon {
    color: var(--destructive, #ef4444);
  }

  .toast-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .toast-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .toast-message {
    font-size: 12px;
    color: var(--foreground, #f3f5f8);
    word-break: break-all;
  }

  .toast-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .toast-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    border-radius: 4px;
    border: 1px solid rgba(34, 197, 94, 0.4);
    background: rgba(34, 197, 94, 0.15);
    color: var(--accent, #22c55e);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.12s;
  }

  .toast-btn:hover {
    background: rgba(34, 197, 94, 0.28);
  }

  .toast-close {
    background: transparent;
    border: none;
    color: var(--muted-foreground, #7e889b);
    cursor: pointer;
    padding: 3px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .toast-close:hover {
    color: var(--foreground, #f3f5f8);
    background: var(--muted, #1c2030);
  }

  @keyframes toastSlide {
    from { opacity: 0; transform: translateY(8px) scale(0.96); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }
</style>
