// Small shared pieces: icon, checkbox, toolbar button, path chip, tag chip.

import { createEffect, createSignal, on, onCleanup, onMount, Show } from "solid-js";
import { splitPath } from "../../../shared/format";
import {
  closeWindow, isWindowMaximized, minimizeWindow, onSnapHover, onWindowResized, setSnapBounds, toggleMaximizeWindow,
} from "../../../shared/os";
import { zoom } from "./zoom";

export function Icon(props: { name: string; fill?: boolean; class?: string }) {
  return <span class={`ms ${props.fill ? "fill" : ""} ${props.class ?? ""}`}>{props.name}</span>;
}

export function Cb(props: { state: boolean | "mixed" }) {
  return (
    <span class="cb" classList={{ on: props.state === true, mixed: props.state === "mixed" }}>
      <Show when={props.state === "mixed"} fallback={<Show when={props.state}><Icon name="check" /></Show>}>
        <Icon name="remove" />
      </Show>
    </span>
  );
}

export function TbBtn(props: {
  icon?: string; label?: string; title?: string; disabled?: boolean; caret?: boolean; on?: boolean; keep?: boolean;
  onClick?: (e: MouseEvent) => void;
}) {
  return (
    <button class="tb-btn" classList={{ lbl: !!props.label, on: props.on }} title={props.title ?? props.label}
      disabled={props.disabled} data-keep={props.keep ? "" : undefined} onClick={(e) => props.onClick?.(e)}>
      <Show when={props.icon}><Icon name={props.icon!} /></Show>
      <Show when={props.label}><span>{props.label}</span></Show>
      <Show when={props.caret}><Icon name="expand_more" class="caret" /></Show>
    </button>
  );
}

export const TagChip = (props: { name: string }) => <span class="tag">{props.name}</span>;

/** A path truncated in the middle, so the drive and the file name both stay visible. */
export function PathChip(props: { path: string }) {
  const parts = () => splitPath(props.path);
  const copy = (e: MouseEvent) => {
    navigator.clipboard?.writeText(props.path).catch(() => {});
    const el = e.currentTarget as HTMLElement;
    el.classList.add("copied");
    setTimeout(() => el.classList.remove("copied"), 1200);
  };
  return (
    <span class="path" title={props.path} onClick={copy}>
      <span class="head">{parts().head}</span><span class="tail">{parts().tail}</span>
    </span>
  );
}

/** Minimise, maximise or restore, close (ADR-0057). Only rendered in the Tauri shell. */
export function WindowControls() {
  const [maximized, setMaximized] = createSignal(false);
  const [snapHot, setSnapHot] = createSignal(false);
  let maximizeBtn!: HTMLButtonElement;
  const refresh = () => void isWindowMaximized().then(setMaximized);
  // The native overlay covers this button (ADR-0057): keep it in step with where the button is.
  const reportBounds = () => {
    const r = maximizeBtn.getBoundingClientRect();
    const s = window.devicePixelRatio;
    setSnapBounds(Math.round(r.left * s), Math.round(r.top * s), Math.round(r.width * s), Math.round(r.height * s));
  };
  onMount(() => {
    refresh();
    reportBounds();
    const stopResize = onWindowResized(() => { refresh(); reportBounds(); });
    const stopHover = onSnapHover(setSnapHot);
    const observer = new ResizeObserver(reportBounds);
    observer.observe(maximizeBtn);
    window.addEventListener("resize", reportBounds);
    // A zoom change moves the button too, not only resizes it (ADR-0077): report on it as well.
    // The class that counter-zooms the bar lands in the same update, so measure after it too (this also
    // covers the first report, which can run before that class is applied).
    createEffect(on(zoom, () => { reportBounds(); requestAnimationFrame(reportBounds); }));
    onCleanup(() => {
      void stopResize.then((unlisten) => unlisten());
      void stopHover.then((unlisten) => unlisten());
      observer.disconnect();
      window.removeEventListener("resize", reportBounds);
    });
  });
  return (
    <div class="winctl">
      <button type="button" aria-label="Minimize" onClick={minimizeWindow}><span class="glyph">{""}</span></button>
      <button type="button" ref={maximizeBtn} classList={{ hot: snapHot() }}
        aria-label={maximized() ? "Restore" : "Maximize"} onClick={toggleMaximizeWindow}>
        <span class="glyph">{maximized() ? "" : ""}</span>
      </button>
      <button type="button" class="close" aria-label="Close" onClick={closeWindow}><span class="glyph">{""}</span></button>
    </div>
  );
}
