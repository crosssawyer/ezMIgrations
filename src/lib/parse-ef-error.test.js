import { describe, it, expect } from "vitest";
import { parseEfError } from "./parse-ef-error";

describe("parseEfError", () => {
  it("returns null for falsy or non-string input", () => {
    expect(parseEfError(null)).toBeNull();
    expect(parseEfError(undefined)).toBeNull();
    expect(parseEfError("")).toBeNull();
    expect(parseEfError(123)).toBeNull();
    expect(parseEfError({})).toBeNull();
  });

  it("returns null when the output has no EF or SqlClient signature", () => {
    expect(parseEfError("dotnet: command not found\nbuild failed\n")).toBeNull();
  });

  it("returns null when EF text is present but contains no migration or SQL detail", () => {
    const raw = "Microsoft.EntityFrameworkCore is loaded\nbut nothing failed here";
    expect(parseEfError(raw)).toBeNull();
  });

  it("extracts the last applying migration mention as failedMigration", () => {
    const raw = [
      "info: Microsoft.EntityFrameworkCore.Migrations[20402]",
      "      Applying migration '20240101_First'.",
      "info: Microsoft.EntityFrameworkCore.Migrations[20402]",
      "      Applying migration '20240102_Second'.",
      "fail: Microsoft.EntityFrameworkCore.Database.Command[20102]",
      "      Failed to apply",
      "Microsoft.Data.SqlClient.SqlException (0x80131904): Invalid column name 'foo'.",
    ].join("\n");

    const result = parseEfError(raw);
    expect(result).not.toBeNull();
    expect(result.failedMigration).toBe("20240102_Second");
    expect(result.failedDirection).toBe("applying");
  });

  it("extracts a reverting migration mention with reverting direction", () => {
    const raw = [
      "info: Microsoft.EntityFrameworkCore.Migrations[20402]",
      "      Reverting migration '20240103_Bad'.",
      "Microsoft.Data.SqlClient.SqlException: cannot revert",
    ].join("\n");

    const result = parseEfError(raw);
    expect(result.failedMigration).toBe("20240103_Bad");
    expect(result.failedDirection).toBe("reverting");
  });

  it("parses SQL exception message into sqlError, stripped of the type prefix", () => {
    const raw = [
      "Applying migration '20240105_Test'.",
      "Microsoft.EntityFrameworkCore.Migrations error",
      "Microsoft.Data.SqlClient.SqlException (0x80131904): Cannot insert duplicate key.",
      "   at SomeStack.Frame()",
    ].join("\n");

    const result = parseEfError(raw);
    expect(result.sqlError).toContain("Cannot insert duplicate key.");
    expect(result.sqlError).not.toMatch(/Microsoft\.Data\.SqlClient\.SqlException/);
  });

  it("falls back to the fail: block when no SqlException is present", () => {
    const raw = [
      "Applying migration '20240106_Foo'.",
      "fail: Microsoft.EntityFrameworkCore.Database.Command[20102]",
      "      The database update could not proceed",
      "      because of a constraint violation",
      "Microsoft.EntityFrameworkCore.DbUpdateException: outer wrapper",
    ].join("\n");

    const result = parseEfError(raw);
    expect(result.sqlError).toContain("The database update could not proceed");
    expect(result.sqlError).toContain("constraint violation");
  });

  it("extracts the offending statement from a Failed executing DbCommand block", () => {
    const raw = [
      "Applying migration '20240107_AddCol'.",
      "fail: Microsoft.EntityFrameworkCore.Database.Command[20102]",
      "      Failed executing DbCommand (12ms) [Parameters=[], CommandType='Text']",
      "      ALTER TABLE [dbo].[Users] ADD [Email] nvarchar(200) NOT NULL;",
      "Microsoft.Data.SqlClient.SqlException: column already exists",
    ].join("\n");

    const result = parseEfError(raw);
    expect(result.statement).toContain("ALTER TABLE [dbo].[Users]");
    expect(result.statement).toContain("[Email]");
  });

  it("truncates long statements with an ellipsis", () => {
    const giantStatement = "X".repeat(2000);
    const raw = [
      "Applying migration '20240108_Big'.",
      "fail: Microsoft.EntityFrameworkCore.Database.Command[20102]",
      "      Failed executing DbCommand",
      "      " + giantStatement,
      "Microsoft.Data.SqlClient.SqlException: too big",
    ].join("\n");

    const result = parseEfError(raw);
    expect(result.statement.length).toBeLessThanOrEqual(1201); // 1200 + ellipsis char
    expect(result.statement.endsWith("…")).toBe(true);
  });

  it("preserves the full unmodified log in fullLog", () => {
    const raw = [
      "Applying migration '20240109_Keep'.",
      "Microsoft.Data.SqlClient.SqlException: keep me intact",
    ].join("\n");

    const result = parseEfError(raw);
    expect(result.fullLog).toBe(raw);
  });

  it("returns the documented shape with all keys", () => {
    const raw = [
      "Applying migration '20240110_Shape'.",
      "Microsoft.Data.SqlClient.SqlException: shape test",
    ].join("\n");

    const result = parseEfError(raw);
    expect(Object.keys(result).sort()).toEqual(
      ["failedMigration", "failedDirection", "sqlError", "statement", "fullLog"].sort()
    );
  });
});
