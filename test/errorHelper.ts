import { TransactionRequest, TransactionResponse } from "ethers";
import { TransactionReceipt } from "ethers";
import { ethers } from "ethers";
import fs from "fs";
import path from "path";

export interface DecodedError {
  contractName: string;
  error: string;
}

export async function tryWait(tx: Promise<TransactionResponse>): Promise<TransactionReceipt | null> {
    try {
        const res = (await tx).wait()
        return res
    } catch(err: any) {
        const data = err?.data ?? err?.error?.data ?? "";
        const decoded = decodeRevert(data);
        console.log(decoded)
        throw new Error()
    }
}

export function decodeRevert(returnData: string): DecodedError {
  if (!returnData || returnData === "0x") {
    return { contractName: "Unknown", error: "Empty revert data" };
  }

  const artifactsDir = path.join(process.cwd(), "artifacts", "contracts");

  type ErrorEntry = { iface: ethers.Interface; contractName: string };
  const selectors: Record<string, ErrorEntry> = {};

  // recursively collect all errors from all ABIs
  const walk = (dir: string) => {
    for (const entry of fs.readdirSync(dir)) {
      const full = path.join(dir, entry);
      if (fs.statSync(full).isDirectory()) {
        walk(full);
      } else if (entry.endsWith(".json")) {
        try {
          const artifact = JSON.parse(fs.readFileSync(full, "utf8"));
          const abi = artifact.abi ?? [];
          const iface = new ethers.Interface(abi);
          const contractName = artifact.contractName ?? path.basename(entry, ".json");
          for (const fragment of iface.fragments) {
            if (fragment.type === "error") {
                const errorFragment = fragment as ethers.ErrorFragment;
                const selector = iface.getError(errorFragment.format())?.selector;
                if (selector) {
                    selectors[selector] = {
                        iface,
                        contractName,
                    };
                }
            }
          }
        } catch {
          // ignore invalid JSON
        }
      }
    }
  };

  walk(artifactsDir);

  // extract 4-byte selector
  const selector = returnData.slice(0, 10);
  const entry = selectors[selector];

  if (!entry) {
    return { contractName: "Unknown", error: `Unrecognized error selector ${selector}` };
  }

  try {
    const parsed = entry.iface.parseError(returnData);
    if (!parsed) {
        throw new Error("Cannot parse error")
    }
    const argsStr = parsed.args.map(a => a.toString()).join(", ");
    return { contractName: entry.contractName, error: `${parsed.name}(${argsStr})` };
  } catch (err) {
    return { contractName: entry.contractName, error: `Failed to decode ${selector}: ${(err as Error).message}` };
  }
}