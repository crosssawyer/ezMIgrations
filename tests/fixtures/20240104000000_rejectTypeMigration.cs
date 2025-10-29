using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace CmmsData.Migrations
{
    /// <inheritdoc />
    public partial class rejecttype : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.AddColumn<int>(
                name: "AcceptAsIsRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                type: "int",
                nullable: true);

            migrationBuilder.AddColumn<int>(
                name: "ReturnRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                type: "int",
                nullable: true);

            migrationBuilder.AddColumn<int>(
                name: "ReworkRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                type: "int",
                nullable: true);

            migrationBuilder.AddColumn<int>(
                name: "ScrapRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                type: "int",
                nullable: true);

            migrationBuilder.CreateTable(
                name: "RejectType",
                schema: "Inventory",
                columns: table => new
                {
                    RejectTypeId = table.Column<int>(type: "int", nullable: false)
                        .Annotation("SqlServer:Identity", "1, 1"),
                    RejectTypeValue = table.Column<string>(type: "nvarchar(max)", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_RejectType", x => x.RejectTypeId);
                });

            migrationBuilder.CreateIndex(
                name: "IX_MriReportLineItemFinalDisposition_AcceptAsIsRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                column: "AcceptAsIsRejectTypeId");

            migrationBuilder.CreateIndex(
                name: "IX_MriReportLineItemFinalDisposition_ReturnRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                column: "ReturnRejectTypeId");

            migrationBuilder.CreateIndex(
                name: "IX_MriReportLineItemFinalDisposition_ReworkRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                column: "ReworkRejectTypeId");

            migrationBuilder.CreateIndex(
                name: "IX_MriReportLineItemFinalDisposition_ScrapRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                column: "ScrapRejectTypeId");

            migrationBuilder.AddForeignKey(
                name: "FK_MriReportLineItemFinalDisposition_RejectType_AcceptAsIsRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                column: "AcceptAsIsRejectTypeId",
                principalSchema: "Inventory",
                principalTable: "RejectType",
                principalColumn: "RejectTypeId");

            migrationBuilder.AddForeignKey(
                name: "FK_MriReportLineItemFinalDisposition_RejectType_ReturnRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                column: "ReturnRejectTypeId",
                principalSchema: "Inventory",
                principalTable: "RejectType",
                principalColumn: "RejectTypeId");

            migrationBuilder.AddForeignKey(
                name: "FK_MriReportLineItemFinalDisposition_RejectType_ReworkRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                column: "ReworkRejectTypeId",
                principalSchema: "Inventory",
                principalTable: "RejectType",
                principalColumn: "RejectTypeId");

            migrationBuilder.AddForeignKey(
                name: "FK_MriReportLineItemFinalDisposition_RejectType_ScrapRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition",
                column: "ScrapRejectTypeId",
                principalSchema: "Inventory",
                principalTable: "RejectType",
                principalColumn: "RejectTypeId");

            migrationBuilder.Sql(@"
                INSERT INTO Inventory.RejectType (RejectTypeValue)
                VALUES 
                    ('Coatings'),
                    ('Weld'),
                    ('Machining'),
                    ('Forming'),
                    ('Material'),
                    ('Assembly'),
                    ('Cut Work');
            ");
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropForeignKey(
                name: "FK_MriReportLineItemFinalDisposition_RejectType_AcceptAsIsRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");

            migrationBuilder.DropForeignKey(
                name: "FK_MriReportLineItemFinalDisposition_RejectType_ReturnRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");

            migrationBuilder.DropForeignKey(
                name: "FK_MriReportLineItemFinalDisposition_RejectType_ReworkRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");

            migrationBuilder.DropForeignKey(
                name: "FK_MriReportLineItemFinalDisposition_RejectType_ScrapRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");

            migrationBuilder.DropTable(
                name: "RejectType",
                schema: "Inventory");

            migrationBuilder.DropIndex(
                name: "IX_MriReportLineItemFinalDisposition_AcceptAsIsRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");

            migrationBuilder.DropIndex(
                name: "IX_MriReportLineItemFinalDisposition_ReturnRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");

            migrationBuilder.DropIndex(
                name: "IX_MriReportLineItemFinalDisposition_ReworkRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");

            migrationBuilder.DropIndex(
                name: "IX_MriReportLineItemFinalDisposition_ScrapRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");

            migrationBuilder.DropColumn(
                name: "AcceptAsIsRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");

            migrationBuilder.DropColumn(
                name: "ReturnRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");

            migrationBuilder.DropColumn(
                name: "ReworkRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");

            migrationBuilder.DropColumn(
                name: "ScrapRejectTypeId",
                schema: "Inventory",
                table: "MriReportLineItemFinalDisposition");
        }
    }
}

