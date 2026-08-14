import * as React from "react";
import { createArkStores, type ArkStores } from "./arkStores";
import { ArkStateContext } from "./arkStateContext";

export function ArkStateProvider({ children }: { children: React.ReactNode }) {
  const storesRef = React.useRef<ArkStores | null>(null);
  if (!storesRef.current) {
    const storedTheme = localStorage.getItem("ark.theme");
    storesRef.current = createArkStores({
      theme: storedTheme === "light" || storedTheme === "dark" ? storedTheme : "dark",
      sidebarCollapsed: localStorage.getItem("ark.sidebar") === "collapsed",
      rightPanelCollapsed: localStorage.getItem("ark.rightPanel") === "collapsed",
    });
  }
  return <ArkStateContext.Provider value={storesRef.current}>{children}</ArkStateContext.Provider>;
}
