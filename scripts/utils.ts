import { ethers, Signer } from "ethers";
import { HardhatEthersSigner } from "@nomicfoundation/hardhat-ethers/types";
import { SEOA__factory } from "../typechain/index.js";

interface UserOp {
    sender: string,
    nonce: bigint,
    initCode: string,
    callData: string,
    preVerificationGas: bigint,
    paymasterAndData: string
    signature: string,
    verificationGasLimit: bigint,
    callGasLimit: bigint,
    maxFeePerGas: bigint,
    maxPriorityFeePerGas: bigint,
}

interface PackedUserOp extends Omit<UserOp, 'verificationGasLimit' | 'callGasLimit' | 'maxFeePerGas' | 'maxPriorityFeePerGas'> {
    accountGasLimits: string,
    gasFees: string,
}

function toBytes32(hex: string): string {
  return ethers.hexlify(ethers.zeroPadValue(ethers.getBytes(hex), 32));
}

export enum CallType {
        Single = '0x00',
        Batch = '0x01',
        DelegateCall = '0xFF'
    }

export enum ExecType {
        Default = '0x00',
        Try = '0x01'
    }

export const encodeMode = (callType: CallType = CallType.Batch, execType: ExecType = ExecType.Default) => {
    // [bytes1 CallType, bytes1 ExecType, bytes4 modeSelector, bytes22 modePayload]
    
    // For default implementation of ERC7821:
    // - callType = Batch (0x01)
    // - execType = Default (0x00)  - fast fail (no try/catch)
    // - modeSelector = 0x00000000 - hell knows what this is for
    // = modePayload = 0x00000000000000000000000000000000000000000000 - hell knows
    
    return ethers.concat([
      callType, // 1
      execType, // 1
      "0x00000000", // 4 
      "0x00000000000000000000000000000000000000000000", // 22
      "0x00000000" // 1 + 1 + 4 + 22 = 28, we're filling missing 4 bytes 
    ]);
}

function packGasLimits(verificationGas: bigint, callGas: bigint): string {
  return ethers.solidityPacked(["uint128", "uint128"], [verificationGas, callGas]);
}

function packGasFees(maxFeePerGas: bigint, maxPriorityFeePerGas: bigint): string {
  return ethers.solidityPacked(["uint128", "uint128"], [maxFeePerGas, maxPriorityFeePerGas]);
}

export function getUserOpHashOffchain(userOp: Omit<UserOp, "signature">, entryPointAddress: string, chainId: bigint) {
    const accountGasLimits = packGasLimits(userOp.verificationGasLimit, userOp.callGasLimit);
    const accountGasLimitsBytes32 = toBytes32(accountGasLimits)
    const gasFees = packGasFees(userOp.maxFeePerGas, userOp.maxPriorityFeePerGas);
    const gasFeesBytes32 = toBytes32(gasFees)

    console.log('eloszki')
    const coder = ethers.AbiCoder.defaultAbiCoder()
    const packedData = coder.encode(
        [
            "address",   // sender
            "uint256",   // nonce
            "bytes32",     // initCode
            "bytes32",     // callData
            "bytes32",   // accountGasLimits (packed)
            "uint256",   // preVerificationGas
            "bytes32",   // gasFees (packed)
            "bytes32"      // paymasterAndData
        ],
        [
            userOp.sender,
            userOp.nonce,
            ethers.keccak256(userOp.initCode),
            ethers.keccak256(userOp.callData),
            accountGasLimitsBytes32,
            userOp.preVerificationGas,
            gasFeesBytes32,
            ethers.keccak256(userOp.paymasterAndData)
        ]
    );
      
      const encoded = coder.encode(
        ["bytes32", "address", "uint256"],
        [ethers.keccak256(packedData), entryPointAddress, chainId]
      );
      
      const userOpHash = ethers.keccak256(encoded);
      
      return userOpHash;
}

export async function getUserOpHashOnchain(userOp: UserOp | PackedUserOp ,entryPointAddress: string, signer: Signer) {
    const entryPointAbi = [
    "function getUserOpHash((address,uint256,bytes,bytes,bytes32,uint256,bytes32,bytes,bytes)) view returns (bytes32)"
  ];
    if (!('accountGasLimits' in userOp) || !('gasFees' in userOp)) {
        userOp = userOpToPackedUserOp(userOp)
    }

    const entryPoint = new ethers.Contract(entryPointAddress, entryPointAbi, signer.provider);

    const userOpHash = await entryPoint.getUserOpHash(userOp);

    return userOpHash
}

export async function signUserOp (signer: Signer, userOp: UserOp, entryPointAddress: string, chainId: bigint) {
    const userOpHash = getUserOpHashOffchain(userOp, entryPointAddress, chainId); 
    return await signer.signMessage(ethers.getBytes(userOpHash));
}

export const userOpToPackedUserOp = (userOp: UserOp): PackedUserOp => {
    const accountGasLimits = packGasLimits(userOp.verificationGasLimit, userOp.callGasLimit);
    const gasFees = packGasFees(userOp.maxFeePerGas, userOp.maxPriorityFeePerGas);
    const {maxFeePerGas, maxPriorityFeePerGas, verificationGasLimit, callGasLimit, ...rest} = userOp
    
    return {
        ...rest,
        accountGasLimits,
        gasFees    
    }
}

export interface IDeployOrGetSmartAccountProps {
    signer: Signer,
    existingAddress?: string,
    ownerAddress?: string,
    whitelistedAddresses?: string[],
    prefundedBalance?: bigint 
}

export interface ExecuteProps {
    target: string,
    value?: bigint,
    data?: string
}

export const encodeExecuteBatch = (executes: ExecuteProps[]) => {
    const executesWithDefaults = executes.map(e => [e.target, e.value ?? 0n, e.data ?? "0x"])
    return ethers.AbiCoder.defaultAbiCoder().encode(["tuple(address target,uint256 value,bytes data)[]"], [executesWithDefaults])
}

export async function checkDelegationStatus(signer: HardhatEthersSigner, verbose: boolean = false): Promise<boolean> {
  verbose && console.log("\n=== CHECKING DELEGATION STATUS ===");



  try {
    const code = await signer.provider?.getCode(signer.address);

    if (code === "0x") {
      verbose && console.log(`❌ No delegation found for ${signer.address}`);
      return false;
    }

    if (code?.startsWith("0xef0100")) {
      const delegatedAddress = "0x" + code.slice(8); // Remove 0xef0100 (8 chars)

      if (verbose) {
        console.log(`✅ Delegation found for ${signer.address}`);
        console.log(`📍 Delegated to: ${delegatedAddress}`);
        console.log(`📝 Full delegation code: ${code}`);
      }

      return true;
    } else {
      verbose && console.log(`❓ Address has code but not EIP-7702 delegation: ${code}`);
      return false;
    }
  } catch (error) {
    verbose && console.error("Error checking delegation status:", error);
    return false;
  }
}

