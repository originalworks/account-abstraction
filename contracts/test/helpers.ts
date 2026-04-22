import { HDNodeWallet, Signer } from "ethers";
import { network } from "hardhat";
const { ethers } = await network.connect();

export interface GetEthersType3WalletsInput {
  fundsSource: Signer;
  numberOfWallets: number;
  prefundValue: bigint;
}

export function randomSalt(): string {
  return ethers.hexlify(ethers.randomBytes(32));
}

export async function futureDeadline(secFromNow: number): Promise<number> {
  const latestBlock = await ethers.provider.getBlock("latest");
  const { timestamp } = latestBlock!;
  return timestamp + secFromNow;
}

export async function getCurrentTimestamp(): Promise<number> {
  const block = await ethers.provider.getBlock("latest");
  return block!.timestamp;
}

export async function setNextBlockTimestamp(timestamp: number) {
  await ethers.provider.send("evm_setNextBlockTimestamp", [timestamp]);
}

// it's necessary to use ethers.Wallet instead of hardhatEthers.Wallet
// as only the first one currently supports type 3 EIP4844 transaction
export async function getEthersType3Wallets(
  input: GetEthersType3WalletsInput,
): Promise<HDNodeWallet[]> {
  let wallets: HDNodeWallet[] = [];
  for (let i = 0; i < input.numberOfWallets; i++) {
    const wallet = ethers.Wallet.createRandom();
    const tx = await input.fundsSource.sendTransaction({
      to: wallet,
      value: input.prefundValue,
    });
    await tx.wait();

    wallets.push(wallet);
  }

  return wallets;
}
