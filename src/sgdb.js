const figlet = require('figlet')
const fs = require('fs')
const handleFile = require('./file')
const DATA_SIZE = 100;

class Sgdb {

    constructor(rootDir) {
        this.databases = {}
        this.currentDb = ''
        this.currentTable = ''
        this.rootDir = rootDir || `${__dirname}/databases`
        this.init()
    }

    init() {
        const instance = fs.existsSync(this.rootDir)

        console.log(figlet.textSync('Fun DB', {
            font: 'Slant Relief',
            horizontalLayout: 'default',
            verticalLayout: 'default'
        }))

        if (!instance) fs.mkdirSync(this.rootDir)

        this.setDependencies()
    }

    listDatabases() {
        const dbs = fs.readdirSync(this.rootDir)
        return dbs
    }

    listTables() {
        const tables = fs.readdirSync(`${this.rootDir}`);
        return tables;
    }

    createTable(table) {
        try {
            fs.openSync(`${this.rootDir}/${table}`, 'w');
            fs.writeFileSync(`${this.rootDir}/${table}.json`, '[]');
            console.log(`Created Table ${table} with success!`);
        } catch (err) {
            console.log(err)
        }
    }

    insert(table, dados) {
        const dbName = this.currentDb
        this.databases[dbName][table].fileSystem.open('r+')
        this.databases[dbName][table].fileSystem.append(dados)
        this.databases[dbName][table].fileSystem.close()
    }

    findByIndex(table, index) {
        const dbName = this.currentDb
        this.databases[dbName][table].fileSystem.open('r+')
        return this.databases[dbName][table].fileSystem.read(index)
    }

    setCurrentTable(table) {
        this.currentTable = table
    }

    setDependencies() {
        const dbs = this.listDatabases()

        dbs.map(dbName => {
            const tables = fs.readdirSync(`${this.rootDir}`)

            this.databases[dbName] = {}

            tables.map(table => {
                this.databases[dbName] = {
                    ...this.databases[dbName],
                    [table]: {
                        url: `${this.rootDir}/${dbName}/${table}`,
                        fileSystem: new handleFile(`${this.rootDir}/${dbName}/${table}`, DATA_SIZE)
                    }
                }
            })
        })
    }
}

module.exports = Sgdb;
