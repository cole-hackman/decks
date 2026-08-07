import { describe, it, expect } from "vitest";
import { formatBytes } from "./bytes";

describe("formatBytes", () => {
  it("keeps small sizes in bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
  });

  it("scales through the binary units", () => {
    expect(formatBytes(5_242_880)).toBe("5.0 MB");
    expect(formatBytes(2_147_483_648)).toBe("2.0 GB");
    expect(formatBytes(1_125_899_906_842_624)).toBe("1024 TB");
  });

  it("drops the decimal once the number is big enough to not need it", () => {
    // 5.0 MB reads as a size; 950 MB reading as "949.7 MB" reads as noise.
    expect(formatBytes(996_147_200)).toBe("950 MB");
  });
});
