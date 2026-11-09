<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    open: boolean;
    title?: string;
    badge?: string;
    badgeType?: "subtle" | "warning" | "accent";
    maxWidth?: string;
    onclose: () => void;
    children: Snippet;
    footer?: Snippet;
  }

  let {
    open = $bindable(false),
    title = "",
    badge = "",
    badgeType = "subtle",
    maxWidth = "540px",
    onclose,
    children,
    footer
  }: Props = $props();

  let mouseDownTarget: EventTarget | null = null;
  let dialogElement: HTMLDivElement | null = $state(null);
  let triggerElement: HTMLElement | null = null;

  function handleBackdropMouseDown(e: MouseEvent) {
    mouseDownTarget = e.target;
  }

  function handleBackdropMouseUp(e: MouseEvent) {
    if (mouseDownTarget === e.currentTarget && e.target === e.currentTarget) {
      close();
    }
    mouseDownTarget = null;
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      close();
    } else if (e.key === "Tab" && dialogElement) {
      // keep Tab inside the dialog
      const items = [...dialogElement.querySelectorAll<HTMLElement>('button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])')].filter(
        (el) => !el.hasAttribute("disabled")
      );
      if (items.length === 0) {
        e.preventDefault();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const current = document.activeElement;
      if (e.shiftKey && (current === first || !dialogElement.contains(current))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (current === last || !dialogElement.contains(current))) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  function close() {
    open = false;
    onclose();
    triggerElement?.focus?.();
  }

  $effect(() => {
    if (open) {
      triggerElement = (document.activeElement as HTMLElement) || null;
      // Focus within modal
      setTimeout(() => {
        const focusable = dialogElement?.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (focusable && focusable.length > 0) {
          focusable[0].focus();
        }
      }, 30);
    }
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="modal-backdrop"
    role="dialog"
    aria-modal="true"
    aria-label={title || "Dialog window"}
    tabindex="-1"
    onmousedown={handleBackdropMouseDown}
    onmouseup={handleBackdropMouseUp}
    onkeydown={handleKeyDown}
  >
    <div
      bind:this={dialogElement}
      class="modal-panel"
      style="max-width: {maxWidth};"
      role="document"
    >
      <div class="modal-header">
        <div class="modal-title-wrap">
          {#if badge}
            <span class="badge badge-{badgeType} font-mono">{badge}</span>
          {/if}
          {#if title}
            <h2 class="modal-title font-mono">{title}</h2>
          {/if}
        </div>
        <button
          type="button"
          class="modal-close-btn"
          aria-label="Close dialog"
          onclick={close}
        >
          <Icon name="x" size={15} />
        </button>
      </div>

      <div class="modal-body">
        {@render children()}
      </div>

      {#if footer}
        <div class="modal-footer">
          {@render footer()}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(3, 5, 8, 0.78);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: var(--z-modal, 70);
    padding: 16px;
    animation: fadeIn 0.14s ease-out;
  }

  .modal-panel {
    width: 100%;
    background: var(--card, #11131c);
    border: 1px solid var(--border-strong, rgba(255, 255, 255, 0.16));
    border-radius: 10px;
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.65), 0 0 0 1px rgba(255, 255, 255, 0.04);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: scaleUp 0.15s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .modal-header {
    padding: 12px 16px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    background: var(--bg-subtle, #0d0f17);
  }

  .modal-title-wrap {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .modal-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--foreground, #f3f5f8);
    word-break: break-all;
  }

  .modal-close-btn {
    background: transparent;
    border: none;
    color: var(--muted-foreground, #7e889b);
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color 0.12s, background-color 0.12s;
  }

  .modal-close-btn:hover {
    color: var(--foreground, #f3f5f8);
    background: var(--muted, #1c2030);
  }

  .modal-body {
    padding: 16px;
    overflow-y: auto;
    max-height: 75vh;
  }

  .modal-footer {
    padding: 10px 16px;
    border-top: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    background: var(--bg-subtle, #0d0f17);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    width: fit-content;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .badge-subtle {
    background: var(--muted, #1c2030);
    color: var(--muted-foreground, #7e889b);
  }

  .badge-warning {
    background: rgba(245, 158, 11, 0.16);
    color: #f59e0b;
    border: 1px solid rgba(245, 158, 11, 0.3);
  }

  .badge-accent {
    background: rgba(34, 197, 94, 0.15);
    color: #22c55e;
    border: 1px solid rgba(34, 197, 94, 0.3);
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes scaleUp {
    from { opacity: 0; transform: scale(0.96) translateY(4px); }
    to { opacity: 1; transform: scale(1) translateY(0); }
  }
</style>
