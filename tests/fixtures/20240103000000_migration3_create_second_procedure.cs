using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace TestProject.Migrations
{
    public partial class CreateUpdateProductsProcedure : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.Sql(@"
                CREATE PROCEDURE UpdateProductPrice
                    @ProductId INT,
                    @NewPrice DECIMAL(18,2)
                AS
                BEGIN
                    UPDATE Products 
                    SET Price = @NewPrice, LastUpdated = GETDATE()
                    WHERE Id = @ProductId
                END;
            ");
        }

        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.Sql(@"DROP PROCEDURE UpdateProductPrice;");
        }
    }
}

