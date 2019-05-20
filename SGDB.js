const figlet = require('figlet')
const fs = require('fs')
const handleFile = require('./FileSystem')
const DATA_SIZE = 100;

class SGDB {

    constructor() {
        this.databases = {}
        this.currentDb = ''
        this.currentTable = ''
        this.rootDir = `${__dirname}/databases`
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

    createDB(name) {
        fs.mkdirSync(`${this.rootDir}/${name}`)
    }

    listDatabases() {
        const dbs = fs.readdirSync(this.rootDir)
        return dbs
    }

    listTables() {
        const dbName = this.currentDb
        const tables = fs.readdirSync(`${this.rootDir}/${dbName}`)
        return tables
    }

    createTable(table) {
        try {
            const dbName = this.currentDb
            fs.openSync(`${this.rootDir}/${dbName}/${table}`, 'w')
            console.log(`Created Table ${table} from ${dbName} with success!`)
            this.setDependencies()
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

    setCurrentDatabase(databaseName) {
        const dbs = this.listDatabases()

        if (!dbs.includes(databaseName)) return false

        this.currentDb = databaseName

        this.setDependencies()

        return databaseName
    }

    setCurrentTable(table) {
        this.currentTable = table
    }

    getCurrentDatabase() {
        return this.currentDb
    }

    setDependencies() {
        const dbs = this.listDatabases()

        dbs.map(dbName => {
            const tables = fs.readdirSync(`${this.rootDir}/${dbName}`)

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

module.exports = new SGDB
