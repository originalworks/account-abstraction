import { network } from "hardhat";
import { SEOA__factory } from "../typechain/factories/SEOA__factory.js";
import { expect } from "chai";
import { HardhatEthersSigner } from "@nomicfoundation/hardhat-ethers/types";
import { SEOA } from "../typechain/SEOA.js";
import { HDNodeWallet } from "ethers";
import { ERC20TokenMock, ERC20TokenMock__factory } from "../typechain/index.js";

const { ethers } = await network.connect();

interface BuildAndSignInput {
  signer: HardhatEthersSigner;
  sEoa: SEOA;
  target: string;
  payload: string;
  salt: string;
  deadline: number;
  chainId?: number;
  value?: number;
}

async function getCurrentTimestamp(): Promise<number> {
  const block = await ethers.provider.getBlock("latest");
  return block!.timestamp;
}

async function setNextBlockTimestamp(timestamp: number) {
  await ethers.provider.send("evm_setNextBlockTimestamp", [timestamp]);
}

function buildERC20MintToPayload(receiver: string, amount: number): string {
  return ERC20TokenMock__factory.createInterface().encodeFunctionData(
    "mintTo",
    [receiver, amount],
  );
}

function randomSalt(): string {
  return ethers.hexlify(ethers.randomBytes(32));
}

async function futureDeadline(secFromNow: number): Promise<number> {
  const latestBlock = await ethers.provider.getBlock("latest");
  const { timestamp } = latestBlock!;
  return timestamp + secFromNow;
}

async function buildRandomBatchInput(
  n: number,
  erc20Mock: ERC20TokenMock,
  signer: HardhatEthersSigner,
  sEoa: SEOA,
): Promise<SEOA.ExecuteInputStruct[]> {
  const target = await erc20Mock.getAddress();
  const deadline = await futureDeadline(60);

  const inputs: SEOA.ExecuteInputStruct[] = [];
  for (let i = 0; i < n; i++) {
    const payload = buildERC20MintToPayload(
      signer.address,
      Math.floor(Math.random() * 1000),
    );
    const salt = randomSalt();

    const signature = await buildAndSign({
      payload,
      target,
      salt,
      deadline,
      signer,
      sEoa,
    });
    inputs.push({
      target,
      payload,
      value: 0,
      salt,
      deadline,
      signature,
    });
  }
  return inputs;
}

async function buildAndSign(input: BuildAndSignInput) {
  const chainId = input.chainId || (await ethers.provider.getNetwork()).chainId;

  const domain = {
    name: "sEOA",
    version: "1",
    chainId,
    verifyingContract: await input.sEoa.getAddress(),
  };

  const types = {
    SignedCall: [
      { name: "target", type: "address" },
      { name: "payloadHash", type: "bytes32" },
      { name: "value", type: "uint256" },
      { name: "salt", type: "bytes32" },
      { name: "deadline", type: "uint256" },
    ],
  };

  const payloadHash = ethers.keccak256(input.payload);

  const value = {
    target: input.target,
    payloadHash,
    value: input.value || 0,
    salt: input.salt,
    deadline: input.deadline,
  };

  return input.signer.signTypedData(domain, types, value);
}

describe("sEOA.sol", () => {
  let deployer: HardhatEthersSigner;
  let gasSponsorA: HardhatEthersSigner;
  let gasSponsorB: HardhatEthersSigner;
  let delegatedAccount: HDNodeWallet;
  let sEOAimplementation: SEOA;
  let erc20Mock: ERC20TokenMock;
  let sEoa: SEOA;

  before(async () => {
    [deployer, gasSponsorA, gasSponsorB] = await ethers.getSigners();
    const sEOA_factory = new SEOA__factory(deployer);
    sEOAimplementation = await sEOA_factory.deploy();
    await sEOAimplementation.waitForDeployment();
    const ERC20TokenMock = await ethers.getContractFactory("ERC20TokenMock");
    erc20Mock = await ERC20TokenMock.deploy();
  });
  beforeEach(async () => {
    delegatedAccount = ethers.Wallet.createRandom().connect(ethers.provider);
    await deployer.sendTransaction({
      to: delegatedAccount,
      value: ethers.parseEther("1"),
    });

    const auth = await delegatedAccount.authorize({
      address: await sEOAimplementation.getAddress(),
      nonce: (await delegatedAccount.getNonce()) + 1,
    });

    const tx = await delegatedAccount.sendTransaction({
      to: delegatedAccount.address,
      authorizationList: [auth],
      data: SEOA__factory.createInterface().encodeFunctionData("usedSalts", [
        "0x0000000000000000000000000000000000000000000000000000000000000000",
      ]),
    });
    await tx.wait();

    sEoa = SEOA__factory.connect(delegatedAccount.address, delegatedAccount);
  });

  describe("execute() — success", function () {
    it("executes a valid signed payload and marks salt as used", async function () {
      const salt = randomSalt();
      const deadline = await futureDeadline(60);

      const payload = buildERC20MintToPayload(delegatedAccount.address, 100);
      const signature = await buildAndSign({
        target: await erc20Mock.getAddress(),
        signer: delegatedAccount,
        sEoa,
        payload,
        salt,
        deadline,
      });

      await expect(
        sEoa.connect(gasSponsorA).execute({
          target: erc20Mock,
          value: 0,
          payload,
          salt,
          deadline,
          signature,
        }),
      )
        .to.emit(sEoa, "Executed")
        .withArgs(salt, gasSponsorA.address, true);

      expect(await sEoa.usedSalts(salt)).to.be.true;
      expect(await erc20Mock.balanceOf(delegatedAccount)).to.equal(100);
    });

    it("any wallet can submit a valid signed payload", async function () {
      const salt1 = randomSalt();
      const salt2 = randomSalt();
      const deadline = await futureDeadline(60);
      const payload = buildERC20MintToPayload(delegatedAccount.address, 100);

      const signature1 = await buildAndSign({
        target: await erc20Mock.getAddress(),
        payload,
        salt: salt1,
        deadline,
        sEoa,
        signer: delegatedAccount,
      });

      const signature2 = await buildAndSign({
        target: await erc20Mock.getAddress(),
        payload,
        salt: salt2,
        deadline,
        sEoa,
        signer: delegatedAccount,
      });

      await expect(
        sEoa.connect(gasSponsorA).execute({
          target: erc20Mock,
          value: 0,
          payload,
          salt: salt1,
          deadline,
          signature: signature1,
        }),
      ).to.emit(sEoa, "Executed");

      await expect(
        sEoa.connect(gasSponsorB).execute({
          target: erc20Mock,
          payload,
          salt: salt2,
          value: 0,
          deadline,
          signature: signature2,
        }),
      ).to.emit(sEoa, "Executed");
    });
  });
  describe("execute() — replay protection", function () {
    it("reverts with AlreadyUsed on salt replay", async function () {
      const salt = randomSalt();
      const payload = buildERC20MintToPayload(delegatedAccount.address, 100);
      const target = await erc20Mock.getAddress();
      const deadline = await futureDeadline(60);
      const signature = await buildAndSign({
        target,
        payload,
        salt,
        deadline,
        signer: delegatedAccount,
        sEoa,
      });

      await sEoa
        .connect(gasSponsorA)
        .execute({ target, value: 0, payload, salt, deadline, signature });

      await expect(
        sEoa
          .connect(gasSponsorA)
          .execute({ target, value: 0, payload, salt, deadline, signature }),
      ).to.be.revertedWithCustomError(sEoa, "AlreadyUsed");
    });
  });
  describe("execute() — deadline", function () {
    it("reverts with Expired when deadline is in the past", async function () {
      const salt = randomSalt();
      const now = await getCurrentTimestamp();
      const deadline = now - 1; // already expired
      const payload = buildERC20MintToPayload(delegatedAccount.address, 100);
      const target = await erc20Mock.getAddress();

      const signature = await buildAndSign({
        payload,
        target,
        signer: delegatedAccount,
        sEoa,
        salt,
        deadline,
      });

      await expect(
        sEoa
          .connect(gasSponsorA)
          .execute({ target, value: 0, payload, salt, deadline, signature }),
      ).to.be.revertedWithCustomError(sEoa, "Expired");
    });

    it("accepts a payload exactly at the deadline block", async function () {
      const salt = randomSalt();
      const deadline = await futureDeadline(5);
      const payload = buildERC20MintToPayload(delegatedAccount.address, 100);
      const target = await erc20Mock.getAddress();

      const signature = await buildAndSign({
        payload,
        target,
        signer: delegatedAccount,
        sEoa,
        salt,
        deadline,
      });

      await setNextBlockTimestamp(deadline);

      await expect(
        sEoa
          .connect(gasSponsorB)
          .execute({ target, value: 0, payload, salt, deadline, signature }),
      ).to.emit(sEoa, "Executed");
    });
  });
  describe("execute() - native coin transfers", () => {
    it("can receive ether in standard transfer", async () => {
      const value = 10000000;
      const tx = await deployer.sendTransaction({ to: sEoa, value });
      const sEoaBalanceBefore = await ethers.provider.getBalance(sEoa);
      await expect(deployer.sendTransaction({ to: sEoa, value })).not.revert(
        ethers,
      );
      const sEoaBalanceAfter = await ethers.provider.getBalance(sEoa);

      expect(sEoaBalanceAfter - sEoaBalanceBefore).to.equal(value);
    });
    it("caller can pass ether in the function call", async () => {
      const value = 1000;
      const salt = randomSalt();
      const deadline = await futureDeadline(60);
      const payload = "0x";
      const target = ethers.Wallet.createRandom().address;

      const signature = await buildAndSign({
        target,
        signer: delegatedAccount,
        sEoa,
        payload,
        salt,
        deadline,
        value,
      });

      const gasSponsorBalanceBefore = await ethers.provider.getBalance(
        gasSponsorA,
      );
      const sEoaBalanceBefore = await ethers.provider.getBalance(sEoa);
      const tx = await sEoa
        .connect(gasSponsorA)
        .execute(
          { target, value, payload, salt, deadline, signature },
          { value },
        );

      const receipt = await tx.wait();
      const gasSponsorBalanceAfter = await ethers.provider.getBalance(
        gasSponsorA,
      );
      const gasCost = receipt!.gasPrice * receipt!.gasUsed;
      const targetBalanceAfter = await ethers.provider.getBalance(target);
      const sEoaBalanceAfter = await ethers.provider.getBalance(sEoa);
      expect(targetBalanceAfter).equal(value);
      expect(sEoaBalanceAfter).equal(sEoaBalanceBefore);
      expect(gasSponsorBalanceBefore - gasSponsorBalanceAfter - gasCost).equal(
        value,
      );
    });
  });
  describe("execute() — signature validation", function () {
    it("reverts with InvalidSignature when signed by wrong key", async function () {
      const salt = randomSalt();
      const deadline = await futureDeadline(60);
      const payload = buildERC20MintToPayload(delegatedAccount.address, 100);
      const target = await erc20Mock.getAddress();

      const signature = await buildAndSign({
        payload,
        target,
        signer: gasSponsorA,
        sEoa,
        salt,
        deadline,
      });

      await expect(
        sEoa
          .connect(gasSponsorA)
          .execute({ target, value: 0, payload, salt, deadline, signature }),
      ).to.be.revertedWithCustomError(sEoa, "InvalidSignature");
    });

    it("reverts when signature is for a different payload", async function () {
      const salt = randomSalt();
      const deadline = await futureDeadline(60);
      const payload1 = buildERC20MintToPayload(delegatedAccount.address, 100);
      const payload2 = buildERC20MintToPayload(delegatedAccount.address, 200);
      const target = await erc20Mock.getAddress();

      const signature = await buildAndSign({
        payload: payload1,
        target,
        signer: delegatedAccount,
        sEoa,
        salt,
        deadline,
      });

      await expect(
        sEoa.connect(gasSponsorA).execute({
          target,
          value: 0,
          payload: payload2,
          salt,
          deadline,
          signature,
        }),
      ).to.be.revertedWithCustomError(sEoa, "InvalidSignature");
    });

    it("reverts when signature is for a different chain", async function () {
      const salt = randomSalt();
      const deadline = await futureDeadline(60);
      const wrongChainId = 123456789;
      const payload = buildERC20MintToPayload(delegatedAccount.address, 200);
      const target = await erc20Mock.getAddress();

      const signature = await buildAndSign({
        payload,
        target,
        signer: delegatedAccount,
        sEoa,
        salt,
        deadline,
        chainId: wrongChainId,
      });

      await expect(
        sEoa
          .connect(gasSponsorA)
          .execute({ target, value: 0, payload, salt, deadline, signature }),
      ).to.be.revertedWithCustomError(sEoa, "InvalidSignature");
    });
  });

  describe("executeBatch()", function () {
    it("executes multiple payloads independently", async function () {
      const batchInput = await buildRandomBatchInput(
        5,
        erc20Mock,
        delegatedAccount,
        sEoa,
      );

      await sEoa.connect(gasSponsorA).executeBatch(batchInput);

      for (const input of batchInput) {
        expect(await sEoa.usedSalts(input.salt)).to.be.true;
      }
    });

    it("halts batch on first failure (execute reverts propagate)", async function () {
      const deadline = await futureDeadline(60);
      const salt1 = randomSalt();
      const salt2 = randomSalt();
      const payload = buildERC20MintToPayload(delegatedAccount.address, 200);
      const target = await erc20Mock.getAddress();

      const signature1 = await buildAndSign({
        payload,
        target,
        signer: delegatedAccount,
        sEoa,
        salt: salt1,
        deadline,
      });

      const signature2 = await buildAndSign({
        payload,
        target,
        signer: gasSponsorB, // wrong signer
        sEoa,
        salt: salt2,
        deadline,
      });

      await expect(
        sEoa.connect(gasSponsorA).executeBatch([
          {
            target,
            payload,
            value: 0,
            salt: salt1,
            deadline,
            signature: signature1,
          },
          {
            target,
            payload,
            value: 0,
            salt: salt2,
            deadline,
            signature: signature2,
          },
        ]),
      ).to.be.revertedWithCustomError(sEoa, "InvalidSignature");

      expect(await sEoa.usedSalts(salt1)).to.be.false;
      expect(await sEoa.usedSalts(salt2)).to.be.false;
    });

    it("batch with one expired deadline reverts entire transaction", async function () {
      const deadline = await futureDeadline(60);
      const expiredDeadline = (await getCurrentTimestamp()) - 1;

      const salt1 = randomSalt();
      const salt2 = randomSalt();
      const payload = buildERC20MintToPayload(delegatedAccount.address, 200);
      const target = await erc20Mock.getAddress();

      const signature1 = await buildAndSign({
        payload,
        target,
        signer: delegatedAccount,
        sEoa,
        salt: salt1,
        deadline,
      });

      const signature2 = await buildAndSign({
        payload,
        target,
        signer: delegatedAccount,
        sEoa,
        salt: salt2,
        deadline: expiredDeadline,
      });

      await expect(
        sEoa.connect(gasSponsorA).executeBatch([
          {
            target,
            payload,
            value: 0,
            salt: salt1,
            deadline,
            signature: signature1,
          },
          {
            target,
            payload,
            value: 0,
            salt: salt2,
            deadline: expiredDeadline,
            signature: signature2,
          },
        ]),
      ).to.be.revertedWithCustomError(sEoa, "Expired");
    });
  });
});
