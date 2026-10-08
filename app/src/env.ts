import { createContext, useContext } from "react";
import type { Environment } from "./api";

/** What this Mac has; loaded once by App. Null until known. */
export const EnvContext = createContext<Environment | null>(null);
export const useEnv = () => useContext(EnvContext);
