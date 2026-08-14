import * as React from "react";
import { ArkStateContext } from "./arkStateContext";

export function useArkStores() {
  const stores = React.useContext(ArkStateContext);
  if (!stores) throw new Error("useArkStores must be used within ArkStateProvider.");
  return stores;
}
