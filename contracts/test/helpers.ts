import { network } from "hardhat";
const { ethers } = await network.connect();

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
