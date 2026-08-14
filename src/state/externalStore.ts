import * as React from "react";

export interface ExternalStore<T> {
  getSnapshot: () => T;
  set: (next: T | ((current: T) => T)) => void;
  subscribe: (listener: () => void) => () => void;
}

/**
 * ARC-008: the smallest store primitive needed for scoped server/UI state. It deliberately has
 * no middleware, reducers, persistence, or action registry: each domain store owns one immutable
 * snapshot and React subscribes through the standard `useSyncExternalStore` contract.
 */
export function createExternalStore<T>(initial: T): ExternalStore<T> {
  let snapshot = initial;
  const listeners = new Set<() => void>();

  return {
    getSnapshot: () => snapshot,
    set(next) {
      const nextSnapshot = typeof next === "function" ? (next as (current: T) => T)(snapshot) : next;
      if (Object.is(snapshot, nextSnapshot)) return;
      snapshot = nextSnapshot;
      listeners.forEach((listener) => listener());
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

export function useStore<T>(store: ExternalStore<T>): T {
  return React.useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot);
}

/**
 * Selectors return a stable field/entity reference from an immutable snapshot. React therefore
 * skips the component when an unrelated entity changes (for example, a stream delta updates one
 * message overlay but every other message bubble, the sidebar, and Settings keep their previous
 * selected snapshot).
 */
export function useStoreSelector<T, S>(store: ExternalStore<T>, selector: (snapshot: T) => S): S {
  const getSelectedSnapshot = React.useCallback(() => selector(store.getSnapshot()), [selector, store]);
  return React.useSyncExternalStore(store.subscribe, getSelectedSnapshot, getSelectedSnapshot);
}
