import { network } from "hardhat";
import { SEOA, SEOA__factory } from "../typechain/index.js";
import {parseEther, Signer, Wallet} from 'ethers'
import { checkDelegationStatus, encodeExecuteBatch, encodeMode, getUserOpHash, PackedUserOp, signUserOp, UserOp, userOpToPackedUserOp } from "./utils.js";
import { EntryPoint__factory } from "../typechain-entrypoint/index.js";
import { DeterministicDeployment } from "./create2.js";

const RPC_URL = "http://127.0.0.1:8545";
const BUNDLER_URL = "http://localhost:14337/rpc"; 
const ENTRYPOINT_V7 = "0x0000000071727De22E5E9d8BAf0edAc6f37da032";

const { ethers } = await network.connect({
    network: "localhost",
    chainType: "l1",
});

const [deployer, donor, external] = await ethers.getSigners();

const owner = new ethers.Wallet("5e1e9d81398b795ae134ac8c85eb6743260175099aa094ad836340fd65387c88", deployer.provider)

const fundOwnerIfNeeded = async () => {
  const ownerBalance = await owner.provider?.getBalance(owner.address)

  if (ownerBalance !== undefined && ownerBalance < ethers.parseEther("1")) {
    await(await donor.sendTransaction({to:owner.address, value: ethers.parseEther("1") - ownerBalance})).wait()
  }
}

const EXECUTOR_ROLE = ethers.id("EXECUTOR_ROLE");

const main = async () => {
  let sEOA;

  await DeterministicDeployment.deploy(new EntryPoint__factory(deployer), {
    salt: "0x90d8084deab30c2a37c45e8d47f49f2f7965183cb6990a98943ef94940681de3",
    contractName: "EntryPoint"
  })

  await fundOwnerIfNeeded()

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
        data: SEOA__factory.createInterface().encodeFunctionData("grantRole", [
                EXECUTOR_ROLE,
                external.address,
              ]),
      })
  
      await tx.wait()

      console.log("New delegation created")
      sEOA = SEOA__factory.connect(owner.address, owner)
  } else {
    console.log("Got existing delegation")
    sEOA = SEOA__factory.connect(owner.address, owner)
  }
    console.log('Going with this shit')
    const {packedUserOp, userOp} = await createUserOp(sEOA)
    await sendUserOpToEntryPoint([packedUserOp], deployer)
}

const createUserOp = async (sEOA: SEOA) => {
    const signer = sEOA.runner as Wallet
    const mode = encodeMode()
    const encodedBatch = encodeExecuteBatch([{target: external.address, value: 1n}])
    const calldata = SEOA__factory.createInterface().encodeFunctionData("execute", [mode, encodedBatch])
    // const nonceKey = // TODO
    const nonce = await sEOA.connect(signer)["getNonce()"]()
    
    // Default
    const userOp: UserOp = {
        sender: await sEOA.getAddress(),
        nonce,
        initCode: "0x",
        callData: calldata,
        verificationGasLimit: 300_000n, // Hardcoded, needs to be estimated
        callGasLimit: 300_000n, // Hardcoded, needs to be estimated
        maxFeePerGas: ethers.parseUnits("1", "gwei"),
        maxPriorityFeePerGas: 0n,
        preVerificationGas: 300_000n, // Hardcoded, needs to be estimated
        paymasterAndData: "0x",
        signature: "0x",
    };

    // Step 2: sign the UserOperation hash
    const packedUserOp = userOpToPackedUserOp(userOp)
    const userOpHash = await getUserOpHash(packedUserOp, ENTRYPOINT_V7, signer.provider!)
    const signature = await signUserOp(userOpHash, signer)
    
    userOp.signature = signature
    packedUserOp.signature = signature

    return {
      userOp,
      packedUserOp
    }
}

const sendUserOpToEntryPoint = async (packedUserOps: PackedUserOp[], sender: Signer) => {
  const entryPointContract = EntryPoint__factory.connect(ENTRYPOINT_V7, sender)

  await(await owner.sendTransaction({to: ENTRYPOINT_V7, value: parseEther('0.5')})).wait()
  const sEOADepositBefore = await entryPointContract.balanceOf(owner.address)
  
  
  const tx = await entryPointContract.handleOps(packedUserOps, await sender.getAddress())
  const receipt = await tx.wait()
  const sEOADepositAfter = await entryPointContract.balanceOf(owner.address)

  const filter = entryPointContract.filters.UserOperationEvent()
  const events = await entryPointContract.queryFilter(filter, receipt?.blockNumber, receipt?.blockNumber)

  for (const event of events) {
    console.log({
      userOpHash: event.args.userOpHash,
      sender: event.args.sender,
      paymaster: event.args.paymaster,
      nonce: event.args.nonce,
      success: event.args.success,
      actualGasCost: event.args.actualGasCost,
      actualGasUsed: event.args.actualGasUsed
    })
  }
}

// TODO
// const sendUserOpToBundler = async (userOps: UserOperation, sEOA: SEOA) => {
//   // const replacer = (_key: string, value: any) =>
//   // typeof value === "bigint" ? "0x" + value.toString(16) : value;

//   // const sendRes = await fetch(BUNDLER_URL, {
//   //   method: "POST",
//   //   headers: { "Content-Type": "application/json" },
//   //   body: JSON.stringify({
//   //     jsonrpc: "2.0",
//   //     id: 2,
//   //     method: "eth_sendUserOperation",
//   //     params: [{...userOp}, ENTRYPOINT]
//   //   }, replacer)
//   // })
//   // console.log("Got response, serializing")

//   // const parsedRes = await sendRes.json() as any

//   // if (parsedRes['error']) {
//   //   console.log(parsedRes['error'])
//   // } else {
//   //     console.log("Send response:", parsedRes);
//   // }

// // const res = await fetch(BUNDLER_URL, {
// //       method: "POST",
// //       headers: { "Content-Type": "application/json" },
// //       body: JSON.stringify({
// //         jsonrpc: "2.0",
// //         id: 3,
// //         method: "eth_getUserOperationReceipt",
// //         params: ["0x34a3835e3e2982026d49bff1501bdec35a435542e2b0c1a590b16fa87262195e"]
// //       })
// //     })

// // const resParsed = await res.json()

// // console.log(resParsed)


// //   const userOpHashSent = parsedRes.result;

// //   // Step 4: poll for UserOperation receipt
// //   let receipt: any;
// //   while (!receipt) {
// //     await new Promise(r => setTimeout(r, 3000));
// //     receipt = await fetch(BUNDLER_URL, {
// //       method: "POST",
// //       headers: { "Content-Type": "application/json" },
// //       body: JSON.stringify({
// //         jsonrpc: "2.0",
// //         id: 3,
// //         method: "eth_getUserOperationReceipt",
// //         params: [userOpHashSent]
// //       })
// //     }).then(r => r.json()).then(j => j.result);

// //     console.log("Polling receipt:", receipt);
// //   }

// //   console.log("Final receipt:", receipt);
// }

void main()