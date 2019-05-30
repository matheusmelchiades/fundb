const fs = require('fs');
const helper = require('./helper');

class Sgdb {

    constructor(rootDir) {
        this.database = {
            system: {}
        }
        this.currentTable = ''
        this.rootDir = rootDir || `${__dirname}/tables`
        this.init()
    }

    init() {
        const instance = fs.existsSync(this.rootDir);

        if (!instance)
            fs.mkdirSync(this.rootDir);
        else
            this.load();
    }

    load() {
        try {
            const files = fs.readdirSync(this.rootDir);

            files.map(file => {
                const table = helper.withOutExt(file);
                const path = `${this.rootDir}/${file}`;
                const data = fs.readFileSync(path, 'utf-8');

                this.database[table] = { path, data: JSON.parse(data) };
            })
        } catch (err) {
            console.log(err)
            console.log('Erro in Load resources fundb!');
        }
    }

    createTable(table) {
        try {
            fs.writeFileSync(`${this.rootDir}/${table}.json`, '[]');

            console.log(`Created Table ${table} with success!`);
        } catch (err) {
            console.log('Error in create table!');
        }
    }

    setCurrentTable(tableName) {
        const tables = Object.keys(this.database)

        if (!tables.includes(tableName)) return false

        this.currentTable = tableName

        return tableName
    }

    getCurrentTable() {
        return this.currentTable
    }

    listTables() {
        try {
            const files = fs.readdirSync(this.rootDir);
            const tables = files.map(helper.withOutExt)
            return tables;
        } catch (err) {
            console.log('Error in Listing tables!')
        }
    }

    insert(dados) {
        try {
            const table = this.database[this.currentTable];
            const id = table.data.length + 1;

            table.data.push({ id, ...dados });

            fs.writeFileSync(table.path, JSON.stringify(table.data, null, 2));

        } catch (err) {
            console.log('Error in insert a new register!')
        }

        this.load()
    }

    findAll() {
        try {
            const table = this.database[this.currentTable]

            return table.data
        } catch (err) {
            console.log('Error in find data!')
        }
    }

    findByIndex(table, index) {
        const dbName = this.currentDb;
        this.databases[dbName][table].fileSystem.open('r+');
        return this.databases[dbName][table].fileSystem.read(index);
    }
}

module.exports = Sgdb;
