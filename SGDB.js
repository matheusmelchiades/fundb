const figlet = require('figlet')
const fs = require('fs')
const handleFile = require('./FileSystem')
const DATA_SIZE = 100;

class SGDB {

    constructor() {
        this.db = {}
        this.currentDb = ''
        this.currentTable = ''
        // this.rootDir = `${__dirname}/databases`
        this.rootDir = `/tmp/databases`
        this.init()
    }

    init() {
        const instance = fs.existsSync(this.rootDir)

        console.log(figlet.textSync('Fun DB', {
            font: 'Slant Relief',
            horizontalLayout: 'default',
            verticalLayout: 'default'
        }))

        if (instance) return

        fs.mkdirSync(this.rootDir)
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
            this.db[dbName] = {
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
        this.db[dbName][table].fileSystem.open('r+')
        this.db[dbName][table].fileSystem.append(dados)
        this.db[dbName][table].fileSystem.close()
    }

    find(table, index) {
        const dbName = this.currentDb
        this.db[dbName][table].fileSystem.open('r+')
        const obj = this.db[dbName][table].fileSystem.read(index)
        console.log(obj)
    }

    setCurrentDatabase(databaseName) {
        const dbs = this.listDatabases()

        if (!dbs.includes(databaseName)) return false

        this.currentDb = databaseName
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
