import * as React from "react";
import type { ArkStores } from "./arkStores";

export const ArkStateContext = React.createContext<ArkStores | null>(null);
