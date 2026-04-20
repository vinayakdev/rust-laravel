<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('users', function (Blueprint $table) {
            $table->string('timezone')->default('UTC');
            $table->string('billing_customer_id')->nullable()->unique();
            $table->timestamp('last_seen_at')->nullable();
            $table->timestamp('marketing_opted_in_at')->nullable();
        });
    }
};
