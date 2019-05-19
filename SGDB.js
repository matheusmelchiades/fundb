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

        if (!instance) return fs.mkdirSync(this.rootDir)

        const dbs = this.listDatabases

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
            this.databases[dbName] = {
                [table]: {
                    fileSystem: new handleFile(`${this.rootDir}/${dbName}/${table}`, DATA_SIZE)
                }
            }
            console.log(`Created Table ${table} from ${dbName} with success!`)
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

        const tables = this.listTables()

        tables.map(table => {
            this.databases[databaseName] = {
                ...this.databases[databaseName],
                [table]: {
                    fileSystem: new handleFile(`${this.rootDir}/${databaseName}/${table}`, DATA_SIZE)
                }
            }
        })

        return databaseName
    }

    setCurrentTable(table) {
        this.currentTable = table
    }

    getCurrentDatabase() {
        return this.currentDb
    }
}

module.exports = new SGDB
