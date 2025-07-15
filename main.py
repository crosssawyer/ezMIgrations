import subprocess
import os
import re
from pathlib import Path


SQLPATTERN = re.compile(
        r'migrationBuilder\.Sql\(\s*(@?"(?:[^"\\]|\\.|"(?=[^)]))+"(?:\s*\+\s*"?[^"]*")*)\s*\)',
        re.DOTALL
)

UP_METHOD_PATTERN = re.compile(
    r"protected override void Up\([^)]*\)\s*\{(.*?)\}", re.DOTALL
)

DOWN_METHOD_PATTERN = re.compile(
    r"protected override void Down\([^)]*\)\s*\{(.*?)\}", re.DOTALL
)

def getDirPathFromUser() -> Path:    
    isComplete: bool = False
    userInput = input("Please enter the path to the migrations folder.\n")
    while isComplete != True:
        path = Path(userInput)
        if path.is_dir():
            return path
        else:
            userInput = input("Please enter a valid path or exit.\n")
            
def getListOfFiles(migrationFilePath: Path) -> list[Path]:
    migrationFiles: list = [
        p for p in migrationFilePath.iterdir()
        if p.is_file() and p.suffix == '.cs' and not p.name.endswith('.Designer.cs')
    ]
    
    if len(migrationFiles) == 0:
        print("This path contains no files. Please enter a valid path.")
        return
    return migrationFiles

def IsStoredProc(customSqlMatch: str):
    return "ALTER PROCEDURE" in customSqlMatch

def getStoredProcedure(customSqlMatch: str): # TODO: finish this method out to grab the stored proc, new method as well for state tracking the storedproc
    this = this
    
        
def checkIfCustomSQL(fileContent: tuple): #TODO: check custom sql, grab from up and down for later migration
    customSqlMatches = SQLPATTERN.findall(fileContent)
    if IsStoredProc(customSqlMatches):
        getStoredProcedure(customSqlMatches)
        
    
def extractUpDownMethods(fileContent: str) -> tuple[str, str]:
    upMethod = UP_METHOD_PATTERN.search(fileContent)
    downMethod = DOWN_METHOD_PATTERN.search(fileContent)
    
    if upMethod == None:
        return
    if downMethod == None:
        return
    
    upContent = upMethod.group(1)
    downContent = downMethod.group(1)
    
    return upContent, downContent

def processFilesPipeline(listOfMigrationFilesL: list):
    for file in listOfMigrationFilesL:
        with file.open("r", encoding="utf-8") as f:
            upDownMethods: tuple = extractUpDownMethods(f.read())
            checkIfCustomSQL(upDownMethods)
            
def main(): # TODO: at somepoint will need to actually run the commands in the cmd line
    migrationsFileDir: Path = getDirPathFromUser()
    listOfMigrationFiles: list = getListOfFiles(migrationsFileDir)
    processFilesPipeline(listOfMigrationFiles)
    
    
if __name__ == "__main__":
    main()