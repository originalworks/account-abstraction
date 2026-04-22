import { loadKZG } from "kzg-wasm";
import fs from "fs";
import { network } from "hardhat";
const { ethers } = await network.connect();

interface CalculatedBlob {
  commitment: string;
  proof: string;
  blobhash: string;
  blobString: string;
}

export class KzgHelper {
  public blobFilePaths = [
    "./test_blobs/1.bin",
    "./test_blobs/2.bin",
    "./test_blobs/3.bin",
  ];
  public calculatedBlobs: CalculatedBlob[] = [];
  blobhashFromCommitment(commitment: string): string {
    return `0x01${ethers.sha256(commitment).slice(4)}`;
  }

  async calculateBlob(
    filePath: string,
    blobToKzgCommitment: (blob: string) => string,
    computeBlobProof: (blob: string, commitment: string) => string,
  ): Promise<CalculatedBlob> {
    const file = fs.readFileSync(filePath);

    const blobString = "0x" + file.toString("hex");

    const commitment = blobToKzgCommitment(blobString);
    const proof = computeBlobProof(blobString, commitment);

    const blobhash = this.blobhashFromCommitment(commitment);

    return { commitment, proof, blobhash, blobString };
  }
  async generate() {
    console.log("Loading KZG setup...");

    const kzg = await loadKZG();

    for (let i = 0; i < this.blobFilePaths.length; i++) {
      const filePath = this.blobFilePaths[i];
      const calculatedBlob = await this.calculateBlob(
        filePath,
        kzg.blobToKzgCommitment,
        kzg.computeBlobProof,
      );

      this.calculatedBlobs.push(calculatedBlob);
    }
  }
}
