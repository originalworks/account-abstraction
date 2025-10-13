import { Provider } from "ethers";
import { network } from "hardhat";
import { SEOA, SEOA__factory } from "../typechain/index.js";
import fetch from "node-fetch";
import {Signer} from 'ethers'
import { checkDelegationStatus, encodeExecuteBatch, encodeMode, signUserOp } from "./utils.js";

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

interface PackedUserOp extends Omit<UserOp, 'accountGasLimits' | 'gasFees'> {
    accountGasLimits: string,
    gasFees: string,
}



const RPC_URL = "http://127.0.0.1:8545";
const BUNDLER_URL = "http://localhost:14337/rpc"; 
const ENTRYPOINT = "0x0000000071727De22E5E9d8BAf0edAc6f37da032";

const { ethers } = await network.connect({
    network: "localhost",
    chainType: "l1",
});

const [deployer, owner, external] = await ethers.getSigners();

const main = async () => {
  let sEOA;
  const hasDelegation = await checkDelegationStatus(owner, true) 
  if (!hasDelegation) {
    const sEOAImplementation = await (await new SEOA__factory(owner).deploy()).waitForDeployment()
    console.log("Implementation deployed")
    
    const auth = await owner.authorize({
        address: await sEOAImplementation.getAddress(),
        nonce: await owner.getNonce() + 1,
      });
    
      const tx = await owner.sendTransaction({
        to: owner.address,
        authorizationList:[auth],
        data: SEOA__factory.createInterface().encodeFunctionData("setAllowedSigners", [[external.address], [true]])
      })
  
      await tx.wait()

      console.log("New delegation created")
      sEOA = SEOA__factory.connect(owner.address, owner)
  } else {
    console.log("Got existing delegation")
    sEOA = SEOA__factory.connect(owner.address, owner)
  }
    console.log('Going with this shit')
    await sendUserOp(external, sEOA)

}

const sendUserOp = async (signer: Signer, sEOA: SEOA) => {
    const chainId = (await signer.provider?.getNetwork())?.chainId!
    const mode = encodeMode()
    const encodedBatch = encodeExecuteBatch([{target: external.address, value: 1n}])
    const calldata = SEOA__factory.createInterface().encodeFunctionData("execute", [mode, encodedBatch])
    // const nonceKey = // LATER
    const nonce = await sEOA.connect(external)["getNonce()"]()
    console.log(`Nonce: ${nonce}`)
    
    // Default
    const userOp: UserOp = {
        sender: await sEOA.getAddress(),
        nonce,
        initCode: "0x",
        callData: calldata,
        verificationGasLimit: 100_000n,
        callGasLimit: 300_000n,
        maxFeePerGas: ethers.parseUnits("1", "gwei"),
        maxPriorityFeePerGas: 0n,
        preVerificationGas: 50_000n,
        paymasterAndData: "0x",
        signature: "0x",
    };

  // Step 2: sign the UserOperation hash
  // The hash = EntryPoint.getUserOpHash(userOp)
    console.log("Creating signature")
    userOp.signature = await signUserOp(signer, userOp, ENTRYPOINT, chainId)

    console.log("sending userOp to the bundler")
  // Step 3: send the UserOperation

  const replacer = (_key: string, value: any) =>
  typeof value === "bigint" ? "0x" + value.toString(16) : value;

  const sendRes = await fetch(BUNDLER_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 2,
      method: "eth_sendUserOperation",
      params: [userOp, ENTRYPOINT]
    }, replacer)
  })
  console.log("Got response, serializing")

  const parsedRes = await sendRes.json() as any

  if (parsedRes['error']) {
    console.log(parsedRes['error'])
  } else {
      console.log("Send response:", parsedRes);
  }

// const res = await fetch(BUNDLER_URL, {
//       method: "POST",
//       headers: { "Content-Type": "application/json" },
//       body: JSON.stringify({
//         jsonrpc: "2.0",
//         id: 3,
//         method: "eth_getUserOperationReceipt",
//         params: ["0x34a3835e3e2982026d49bff1501bdec35a435542e2b0c1a590b16fa87262195e"]
//       })
//     })

// const resParsed = await res.json()

// console.log(resParsed)


//   const userOpHashSent = parsedRes.result;

//   // Step 4: poll for UserOperation receipt
//   let receipt: any;
//   while (!receipt) {
//     await new Promise(r => setTimeout(r, 3000));
//     receipt = await fetch(BUNDLER_URL, {
//       method: "POST",
//       headers: { "Content-Type": "application/json" },
//       body: JSON.stringify({
//         jsonrpc: "2.0",
//         id: 3,
//         method: "eth_getUserOperationReceipt",
//         params: [userOpHashSent]
//       })
//     }).then(r => r.json()).then(j => j.result);

//     console.log("Polling receipt:", receipt);
//   }

//   console.log("Final receipt:", receipt);
}

void main()