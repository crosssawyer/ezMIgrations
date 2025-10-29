using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace TestProject.Migrations
{
    public partial class AlterGetProductsProcedure : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.Sql(@"
                ALTER PROCEDURE GetProducts
                AS
                BEGIN
                    SELECT Id, Name, Price FROM Products WHERE Price > 0
                END;
            ");
        }

        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.Sql(@"
                ALTER PROCEDURE GetProducts
                AS
                BEGIN
                    SELECT * FROM Products
                END;
            ");
        }
    }
}

