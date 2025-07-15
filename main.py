import subprocess
import os
import re
from pathlib import Path

def getDirPathFromUser():    
    isComplete: bool = False
    userInput = input("Please enter the path to the migrations folder.\n")
    while isComplete != True:
        path = Path(userInput)
        if path.is_dir():
            return path
        else:
            userInput = input("Please enter a valid path or exit.\n")
            
def grabListOfFiles(migrationFilePath: Path):
    migrationFiles: list = [
        p for p in migrationFilePath.iterdir()
        if p.is_file() and p.suffix == '.cs' and not p.name.endswith('.Designer.cs')
    ]
    
    if len(migrationFiles) == 0:
        print("This path contains no files. Please enter a valid path.")
        return
    return migrationFiles
        
def checkIfCustomSQL(fileContent: str):
    sqlPattern = re.compile(
            r'migrationBuilder\.Sql\(\s*(@?"(?:[^"\\]|\\.|"(?=[^)]))+"(?:\s*\+\s*"?[^"]*")*)\s*\)',
            re.DOTALL
    )
    customSqlMatches = sqlPattern.findall(fileContent)  
    print(customSqlMatches)
    
def processEachFile(listOfMigrationFilesL: list):
    for file in listOfMigrationFilesL:
        with file.open("r", encoding="utf-8") as f:
            checkIfCustomSQL(f.read())
                
            
def main():
    migrationsFileDir: Path = getDirPathFromUser()
    listOfMigrationFiles: list = grabListOfFiles(migrationsFileDir)
    processEachFile(listOfMigrationFiles)
    
    
if __name__ == "__main__":
    main()