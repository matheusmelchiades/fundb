const Fs = require('fs');


class File {

    fd;
    fillString = ' ';
    path;
    dataSize;

    constructor(path, dataSize) {
        this.path = path;
        this.dataSize = dataSize;
    }

    open(flags = 'w') {

        this.fd = Fs.openSync(this.path, flags);

    }

    close() {

        if (this.fd) {

            Fs.closeSync(this.fd);

            this.fd = null;

        }

    }

    read(index = 0) {

        this.checkIsOpen();

        const position = this.dataSize * index;

        const buffer = Buffer.alloc(this.dataSize);

        const bytesRead = Fs.readSync(this.fd, buffer, 0, this.dataSize, position);

        if (!bytesRead)
            return null;

        const obj = this.toJson(buffer);

        return obj;

    }

    toJson(dataBuffer) {

        const data = dataBuffer.toString();

        const stringify = data.replace(new RegExp(this.fillString, 'g'), '');

        const obj = JSON.parse(stringify);

        return obj;

    }

    append(obj) {

        this.checkIsOpen();

        const stringify = JSON.stringify(obj);

        this.checkDataSize(stringify);

        const buffer = this.toPadedBuffer(stringify);

        Fs.appendFileSync(this.path, buffer);

    }

    checkIsOpen() {

        if (!this.fd)
            throw new Error('file isn`t opened');

    }

    checkDataSize(data) {

        if (data.length > this.dataSize)
            throw new Error(`data length overflow, max(${this.dataSize.toString()})`);

    }

    toPadedBuffer(stringify) {

        const data = stringify.padEnd(this.dataSize, this.fillString);

        const buffer = Buffer.from(data);

        return buffer;

    }

    edit(obj, index) {

        const position = this.dataSize * index;

        this.checkIsOpen();

        const stringify = JSON.stringify(obj);

        this.checkDataSize(stringify);

        const buffer = this.toPadedBuffer(stringify);

        Fs.writeSync(this.fd, buffer, 0, this.dataSize, position);

    }
}

module.exports = File;